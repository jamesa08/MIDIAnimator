from __future__ import annotations
from typing import Tuple, List, Dict, Union, Optional
from mathutils import Vector, Euler
from dataclasses import dataclass
from numpy import add as npAdd
from ..utils.blender import *
from ..data_structures.midi import MIDINote
import bpy

@dataclass
class Keyframe:
    """simple keyframe data structure"""
    frame: float  # frame of the keyframe
    value: float  # value of the keyframe

    def __hash__(self) -> int:
        return hash((self.frame, self.value))

@dataclass(init=False)
class BlenderWrapper:
    """object wrapper for a Blender Object
    this allows us to store data with the blender objects (such as FCurve data, note numbers, MIDI information, etc)
    """

    obj: bpy.types.Object
    noteNumbers: Tuple[int]
    
    noteOnCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]]
    noteOffCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]]

    keyframes: ObjectKeyframes

    def __init__(self, obj: bpy.types.Object, noteNumbers: tuple, noteOnCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]], noteOffCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]]):
        self.obj = obj
        
        self.noteNumbers = noteNumbers
        self.noteOnCurves = sorted(noteOnCurves, key=lambda x: f"{x.data_path}_{x.array_index}")
        self.noteOffCurves = sorted(noteOffCurves, key=lambda x: f"{x.data_path}_{x.array_index}")

        # make sure object does not use itself as a note on/off curve for the keyframed type
        # make sure obj has at least a note on/off curve for the keyframed type
        # make sure the FCurves are valid
        if obj.midi.anim_type == "keyframed":
            assert obj.midi.note_on_curve != obj and obj.midi.note_off_curve != obj, f"Object '{obj.name}' cannot use itself as an animation curve (Note On/Off)!"
            assert obj.midi.note_on_curve is not None or obj.midi.note_off_curve is not None, f"Object '{obj.name}' does not have a Note On/Off animation curve! To use the Keyframed Animation type, you need to have a Note On curve or a Note Off curve."
            assert validateFCurves(noteOnCurves, noteOffCurves) is not False, "Note On FCurve object and the Note Off FCurve object have the different data paths (or extraneous data paths)! Make sure to match the Note On and Note Off data paths."
        
        self.keyframes = ObjectKeyframes(wpr=self)
        self.noteOnKeyframes = ObjectKeyframes(wpr=self)
        self.noteOffKeyframes = ObjectKeyframes(wpr=self)
        
        # these need to be copied at the first keyframe
        self.initalLoc = obj.location.copy()
        self.initalRot = obj.rotation_euler.copy()
        self.initalScl = obj.scale.copy()

@dataclass
class ObjectShapeKey:
    name: str = ""
    referenceCurve: bpy.types.FCurve = None
    targetKey: bpy.types.ShapeKey = None

    data_path: str = referenceCurve.data_path if referenceCurve is not None else ""
    array_index: int = 0

    def __hash__(self) -> int:
        return hash((self.referenceCurve, self.targetKey))

@dataclass(init=False)
class ObjectKeyframes:
    wpr: BlenderWrapper

    # key: Tuple[FCurve.data_path, FCurve.array_index], value: List[Keyframe]
    listOfKeys: Dict[Tuple[str, int], List[Keyframe]]

    def __init__(self, wpr):
        self.wpr = wpr
        self.listOfKeys = {}

@dataclass
class FrameRange:
    """
    this stores an object that will be moving from startFrame to endFrame
    """
    startFrame: int
    endFrame: int
    bObj: BlenderWrapper

    def __post_init__(self):
        self.cachedObj: Optional[bpy.types.Object] = None

    def __lt__(self, other: FrameRange) -> FrameRange:
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
