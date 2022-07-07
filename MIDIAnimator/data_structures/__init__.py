from __future__ import annotations
import bpy
from dataclasses import dataclass
from typing import Tuple, List, Dict, Union, Optional
from mathutils import Vector, Euler
from ..utils.blender import *

class NoteAnimator:
    obj: bpy.types.Object
    animators: ObjectFCurves

    # how many frames before a note is hit does this need to start animating
    _frameStartOffset: float
    # how many frames after a note is hit does it continue animating
    _frameEndOffset: float

    def __init__(self, obj: bpy.types.Object, animators: ObjectFCurves):
        self.obj = obj
        self.animators = animators
        # temporary values until we compute in _calculateOffsets()
        self._frameStartOffset = 0.0
        self._frameEndOffset = 0.0

        self._calculateOffsets()

    def _calculateOffsets(self):
        # when playing a note, calculate the offset from the note hit time to the earliest animation for the note
        # and the latest animation for the note
        combined = self.animators.location + self.animators.rotation + self.animators.material + self.animators.shapeKeys
        start, end = None, None
        for fCrv in combined:
            curveStart, curveEnd = fCrv.range()
            if start is None or curveStart < start:
                start = curveStart
            if end is None or curveEnd > end:
                end = curveEnd
        self._frameStartOffset = start
        self._frameEndOffset = end

    def frameOffsets(self) -> Tuple[float, float]:
        """
        :return: start (probably negative) and end offsets for playing the note
        """
        return self._frameStartOffset, self._frameEndOffset

@dataclass(init=False)
class ObjectFCurves:
    location: Tuple[bpy.types.FCurve]
    rotation: Tuple[bpy.types.FCurve]
    material: Tuple[bpy.types.FCurve]
    
    # key= the "to keyframe" object's shape key's name, value= a list \
    # (index 0 = reference object shape key FCurves, index 1 = the "to keyframe" object's shape key)
    shapeKeysDict: Dict[str, List[bpy.types.FCurve, bpy.types.ShapeKey]]
    shapeKeys: Tuple[bpy.types.FCurve]  # a list of the reference object's shape keys FCurves

    def __init__(self, location = (), rotation = (), shapeKeysDict = (), shapeKeys = (), material = ()):
        self.location = location
        self.rotation = rotation
        self.shapeKeysDict = shapeKeysDict
        self.shapeKeys = shapeKeys
        self.material = material

class FCurveProcessor:
    obj: bpy.types.Object
    locationObject: Optional[bpy.types.Object]
    fCurves: ObjectFCurves
    location: Optional[Vector]
    rotation: Optional[Euler]
    
    # key= custom property name, value=int or float (the val to be keyframed)
    material: Dict[str, Union[int, float]]
    
    # key= the "to keyframe" object's shape key's name, value= a float (the val to be keyframed)
    shapeKeys: Dict[str, float]

    def __init__(self, obj: bpy.types.Object, fCurves: ObjectFCurves, locationObject: Optional[bpy.types.Object] = None):
        self.obj = obj
        self.fCurves = fCurves
        self.locationObject = locationObject
        # when None no keyframe of that type
        self.location = None
        self.rotation = None
        self.material = {}
        self.shapeKeys = {}

    def applyFCurve(self, delta: int):
        # for fCurve in self.fCurves.location:    
            # do the work in BlenderLocationKeyFrame and set self.location
        if len(self.fCurves.location) != 0:
            if self.locationObject is None:
                location = self.obj.location.copy()
            else:
                location = self.locationObject.location.copy()

            for fCurve in self.fCurves.location:
                i = fCurve.array_index
                val = fCurve.evaluate(delta)
                location[i] = val
            
            # set the values on internal location
            self.location = location

        if len(self.fCurves.rotation) != 0:
            # do the work in BlenderRotationKeyFrame and add to self.rotation
            if self.rotation is None:
                rotation = self.obj.rotation_euler.copy()
            else:
                rotation = self.rotation

            for fCurve in self.fCurves.rotation:
                i = fCurve.array_index
                val = fCurve.evaluate(delta)
                rotation[i] += val

            # set the values on internal rotation
            self.rotation = rotation

        if len(self.fCurves.material) != 0:
            for fCurve in self.fCurves.material:
                val = fCurve.evaluate(delta)
                if fCurve.data_path in self.material:
                    self.material[fCurve.data_path] += val
                else:
                    self.material[fCurve.data_path] = val

        if len(self.fCurves.shapeKeysDict) != 0:
            for shapeName in self.fCurves.shapeKeysDict:
                val = self.fCurves.shapeKeysDict[shapeName][0].evaluate(delta)  # we want to get only the FCurve from the dict (index 0 is the FCurve)

                if shapeName in self.shapeKeys:
                    self.shapeKeys[shapeName] += val
                else:
                    self.shapeKeys[shapeName] = val

    def insertKeyFrames(self, frame: int):
        if self.location is not None:
            # make the deta location keyframe for self.obj
            self.obj.location = self.location
            self.obj.keyframe_insert(data_path="location", frame=frame)
            setInterpolationForLastKeyframe(self.obj, "BEZIER")
        
        if self.rotation is not None:
            self.obj.delta_rotation_euler = self.rotation
            self.obj.keyframe_insert(data_path="delta_rotation_euler", frame=frame)
        
        if self.material is not None:
            for data_path in self.material:
                val = self.material[data_path]
                exec(f"bpy.context.scene.objects['{self.obj.name}']{data_path} = {val}")
                self.obj.keyframe_insert(data_path=data_path, frame=frame)

        if self.shapeKeys is not None:
            for shapeName in self.fCurves.shapeKeysDict:
                shapeKey = self.fCurves.shapeKeysDict[shapeName][1]  # index 1 is the shape Key to keyframe

                shapeKey.value = self.shapeKeys[shapeName]
                shapeKey.keyframe_insert(data_path="value", frame=frame)

@dataclass
class FrameRange:
    """
    this stores an object that will be moving from _startFrame to _endFrame
    """
    startFrame: int
    endFrame: int
    obj: Optional[bpy.types.Object]

    def __post_init__(self):
        self.cachedObj: Optional[bpy.types.Object] = None

    def __lt__(self, other: FrameRange):
        return self.startFrame < other.startFrame

@dataclass(init=False)
class CacheInstance:
    _cache = List[bpy.types.Object]

    def __init__(self, objs: List[bpy.types.Object]) -> None:
        """initializes a type List[bpy.types.Object] to the cache instance."""
        self._cache = objs

    def returnObject(self, obj: bpy.types.Object) -> None:
        """returns a bpy.types.Object back to the cache. This is the method to use when you are done with the object"""
        self._cache.append(obj)
    
    def getObject(self) -> bpy.types.Object:
        """takes out a bpy.types.Object from the cache. This is the method to use when you want to take an object out
        
        :return: the reusable bpy.types.Object
        """
        obj = self._cache.pop()
        return obj
