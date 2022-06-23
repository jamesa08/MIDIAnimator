from __future__ import annotations
import bpy
from dataclasses import dataclass
from math import floor, ceil
from typing import Dict, List, Tuple, Optional, Union, TYPE_CHECKING
from mathutils import Vector, Euler

from .. utils.blender import *

if TYPE_CHECKING:
    from .. src.MIDIStructure import MIDITrack

def maxSimultaneousObjects(intervals: List[FrameRange]) -> int:
    """
    :param intervals: List[FrameRange]
    :return int: max number of objects that are visible at any point in time
    """
    # keep track of maximum number of active items
    maxCount = 0

    # number of active items currently
    activeCount = 0
    # list of end times for currently active items sorted by the end time
    endTimesForActive = []

    # for testing code
    # currentActives = []

    # for each (start frame, end frame) interval for objects
    for frameRange in intervals:
        start = frameRange.startFrame
        end = frameRange.endFrame
        endIndex = 0
        endTimesCount = len(endTimesForActive)
        # remove active objects whose end time is before this new interval we are processing
        # while end times left to check and the end time is before the start time for this interval
        while endIndex < endTimesCount and endTimesForActive[endIndex] < start:

            # testing code to output intermediate steps
            # for (i, (x, y)) in enumerate(currentActives):
            #     if y == endTimesForActive[endIndex]:
            #         del currentActives[i]
            #         break

            # this one has ended so move to next index and decrease current active count
            endIndex += 1
            activeCount -= 1


        # remove all the ones from list whose end time is earlier than the start of this interval
        endTimesForActive = endTimesForActive[endIndex:]

        # testing code
        # currentActives.append((start, end))
        # print(start, end, currentActives)

        # add the item for this interval
        activeCount += 1
        # update maxCount if new maximum
        if activeCount > maxCount:
            maxCount = activeCount

        # put it in the list of endTimesForActive objects
        endTimesForActive.append(end)
        # position it in sorted order using insertionSort algorithm
        # this should be efficient since in general the new end times will usually go at end of list
        pos = len(endTimesForActive) - 1
        newItem = endTimesForActive[pos]
        while pos > 0 and endTimesForActive[pos - 1] > newItem:
            endTimesForActive[pos] = endTimesForActive[pos - 1]
            pos -= 1
        endTimesForActive[pos] = newItem

    # after processing all intervals return the computed maximum active count
    return maxCount

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
        self.material = None
        self.shapeKeys = None

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
            self.obj.delta_location = self.location
            self.obj.keyframe_insert(data_path="delta_location", frame=frame)
        
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

@dataclass
class Instrument:
    """base class for instruments that are played for notes"""
    collection: bpy.types.Collection
    midiTrack: 'MIDITrack'
    noteToObjTable: Dict[int, bpy.types.Object]
    _activeObjectList: List[FrameRange]
    _activeNoteDict: Dict[int, List[FrameRange]]
    _frameStart: int
    _frameEnd: int
    _objFrameRanges: List[FrameRange]

    def __init__(self, midiTrack: 'MIDITrack', collection: bpy.types.Collection):
        self.collection = collection
        self.midiTrack = midiTrack
        self.noteToObjTable = dict()
        self.override = False
        self._activeObjectList = []
        self._activeNoteDict = dict()
        self._frameStart = 0
        self._frameEnd = 0
        self._objFrameRanges = []

        result = self._makeObjToFCurveDict()
        self.createNoteToObjTable(result)

    def _makeObjToFCurveDict(self) -> Dict[bpy.types.Object, ObjectFCurves]:
        fCurveDict = {}
        for obj in self.collection.all_objects:
            location = []
            rotation = []
            shapeKeysDict = {}
            shapeKeys = []
            material = []
            for fCrv in FCurvesFromObject(obj.animation_curve):
                dataPath = fCrv.data_path
                if dataPath == "location":
                    location.append(fCrv)
                elif dataPath == "rotation_euler":
                    rotation.append(fCrv)
                elif dataPath[:2] == '["' and dataPath[-2:] == '"]':  # this is a custom property that we're finding
                    getType = eval(f"type(bpy.context.scene.objects['{obj.animation_curve.name}']{dataPath})")
                    assert getType == float or getType == int, "Please create type `int` or type `float` custom properties"
                    material.append(fCrv)
            
            # first need to get all of this reference object's shape key FCurves
            for fCrv in shapeKeyFCurvesFromObject(obj.animation_curve):
                if fCrv.data_path[-5:] == "value":  # we only want it if they have keyframed "value"
                    # fCrv.data_path returns 'key_blocks["name"].value'.
                    # 'key_blocks["' will never change and so will '"].value'.
                    # chopping those off gives us just the name
                    
                    name = fCrv.data_path[12:-8]
                    shapeKeysDict[name] = [fCrv]  # always 1 FCurve for 1 shape key

            # now get this object's shape keys so we can insert keyframes on them
            for shpKey in shapeKeysFromObject(obj)[0]:  #only want the shape keys, not the basis, see func for more info
                if shpKey.name in shapeKeysDict:
                    shapeKeysDict[shpKey.name].append(shpKey)
            
            # delete unused shape keys (these are the keys that would be on the reference object)
            for key in shapeKeysDict.keys():
                val = shapeKeysDict[key]
                if len(val) != 2: 
                    del shapeKeysDict[key]
                    continue
                
                shapeKeys.append(val[0])  # add the FCurve only to the internal list of shapeKeys
            
            fCurveDict[obj] = ObjectFCurves(tuple(location), tuple(rotation), shapeKeysDict, tuple(shapeKeys), tuple(material))

        return fCurveDict

    def createNoteToObjTable(self, fCurveDict: Dict[bpy.types.Object, ObjectFCurves]) -> None:
        for obj in self.collection.all_objects:
            if obj.note_number is None: raise RuntimeError(f"Object '{obj.name}' has no note number!")
            if int(obj.note_number) in self.noteToObjTable: raise RuntimeError(
                f"There are two objects in the scene with duplicate note numbers.")
            na = NoteAnimator(obj, fCurveDict[obj])
            self.noteToObjTable[int(obj.note_number)] = na

    def preAnimate(self):
        pass

    def preFrameLoop(self):
        # copy the frames to not mutate them, then sort by start time and then reverse
        self.createFrameRanges()
        self.preAnimate()
        self._objFrameRanges.sort(reverse=True)

        self._frameStart, self._frameEnd = self._objFrameRanges[-1].startFrame, self._objFrameRanges[0].endFrame

    def postFrameLoop(self):
        self.noteToObjTable = dict()
        self._activeObjectList = []
        self._activeNoteDict = dict()
        self._objFrameRanges = []

    def animateFrames(self, offset: int):
        """create keyframes for the animation

        :param int offset: amount to add to the frame number for each keyframe (in case we have negative keyframes)
        """
        for frame in range(self._frameStart, self._frameEnd):
            self.updateActiveObjectList(frame, offset)
            
            # update activeNoteDict: the notes that are still playing

            # add a keyframe for each object that is moving during this frame
            # call animate method to insert a keyframe
            # instead of passing activeObjectList, pass in activeNoteDict
            self.animate(frame, offset)

    def updateActiveObjectList(self, frame: int, offset: int):
        # variables
        cache: Optional[CacheInstance] = None
        if hasattr(self, '_cacheInstance'):
            cache = self._cacheInstance

        stillActiveList = []
        # delete/return to cache for old objects
        for frameInfo in self._activeObjectList:
            objEndFrame = frameInfo.endFrame
            obj = frameInfo.obj
            cachedObj = frameInfo.cachedObj

            if objEndFrame >= frame:
                stillActiveList.append(frameInfo)
            else:
                # this note is no longer being played
                # remove from activeNoteDict
                self._activeNoteDict[obj.note_number].remove(frameInfo)

                # return object to cache and hide it
                if cache is not None and cachedObj is not None:
                    cache.returnObject(cachedObj)

                    # disable cached object in viewport/render
                    showHideObj(cachedObj, True, frame + offset)

        self._activeObjectList = stillActiveList

        # update activeObjectList with new objects
        i = len(self._objFrameRanges) - 1

        while i >= 0 and self._objFrameRanges[i].startFrame <= frame:
            frameInfo = self._objFrameRanges[i]
            obj = frameInfo.obj

            # if we have a cache, get reusable object
            if cache is not None:
                cachedObj = cache.getObject()
                # tell the frameInfo to use the cached object
                frameInfo.cachedObj = cachedObj
                # enable cached object in viewport/render
                showHideObj(cachedObj, False, frame + offset)

            # this object is now being animated until we reach its end frame
            self._activeObjectList.append(frameInfo)

            # update dictionary that keeps track of notes currently being animated
            if obj.note_number in self._activeNoteDict:
                self._activeNoteDict[obj.note_number].append(frameInfo)
            else:
                self._activeNoteDict[obj.note_number] = [frameInfo]

            # we added it to active list so have next iteration check next frame range in reverse sorted order
            self._objFrameRanges.pop()
            i -= 1

    def createFrameRanges(self):
        assert self.noteToObjTable is not None, "please run createNoteToObjTable first"
        result = []

        for note in self.midiTrack.notes:
            # lookup obj from note number
            noteAnimator = self.noteToObjTable[note.noteNumber]
            obj = noteAnimator.obj

            try:
                hit = obj.note_hit_time
            except AttributeError:
                print(f"{obj.name} has no hit time!")
                hit = 0

            frame = int(secToFrames(note.timeOn))
            offsets = noteAnimator.frameOffsets()
            startFrame = int(floor(offsets[0] - hit + frame))
            endFrame = int(ceil(offsets[1] - hit + frame))
            result.append(FrameRange(startFrame, endFrame, obj))

        self._objFrameRanges = result

    def animate(self, frame: int, offset: int):
        # each cached object needs a separate FCurveProcessor since the same note can be "in progress" more than
        # once for a given frame
        cacheObjectProcessors = {}

        # for this note, iterate over all frame ranges for the note being played with objects still moving
        for noteNumber in self._activeNoteDict:
            # if finished playing all instances of this note, skip to next note
            if len(self._activeNoteDict[noteNumber]) <= 0:
                continue

            # for non projectiles, this will be the same object each through the inner for loop
            frameInfo = self._activeNoteDict[noteNumber][0]
            obj = frameInfo.obj

            # set of FCurveProcessor in case more than one cached object currently in-progress for the same note
            processorSet = set()

            # get the ObjectFCurves for this note
            objFCurve = self.noteToObjTable[int(obj.note_number)].animators
            # make a processor for it
            processor = FCurveProcessor(obj, objFCurve)
            # keep track of all objects that need keyframed
            processorSet.add(processor)
            
            for frameInfo in self._activeNoteDict[noteNumber]:
                # check if we are animating a cached object for this
                cachedObj = frameInfo.cachedObj
                if cachedObj is not None:
                    # if not the first time we are using this cachedObject, get its FCurveProcessor
                    if cachedObj in cacheObjectProcessors:
                        processor = cacheObjectProcessors[cachedObj]
                    # this is the first time this cachedObject is used so need to create its FCurveProcessor
                    else:
                        # create a processor and include in set for keyframing
                        processor = FCurveProcessor(cachedObj, objFCurve, obj)
                        processorSet.add(processor)
                        cacheObjectProcessors[cachedObj] = processor

                # accumulate the FCurve results for this object
                # need this step for cymbals since we need to add the FCurves for each instance of the note
                objStartFrame = frameInfo.startFrame
                delta = frame - objStartFrame
                processor.applyFCurve(delta)

            # for each object (could be multiple cached objects) insert the key frame
            for p in processorSet:
                p.insertKeyFrames(frame + offset)

class ProjectileInstrument(Instrument):
    _cacheInstance: Optional[CacheInstance]

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

        # post init things here
        self._cacheInstance = None

    def preAnimate(self):
        # delete old objects
        objectCollection = self.collection

        assert objectCollection.projectile_collection is not None, "Please define a Projectile collection for type Projectile."
        assert objectCollection.reference_projectile is not None, "Please define a reference object to be duplicated."
        cleanCollection(objectCollection.projectile_collection, objectCollection.reference_projectile)

        # calculate number of needed projectiles & instance the blender objects using bpy
        maxNumOfProjectiles = maxSimultaneousObjects(self._objFrameRanges)

        projectiles = []

        for i in range(maxNumOfProjectiles):
            duplicate = objectCollection.reference_projectile.copy()
            duplicate.name = f"projectile_{hex(id(self.midiTrack))}_{i}"
            
            # hide them
            showHideObj(duplicate, True, self._objFrameRanges[0].startFrame)

            objectCollection.projectile_collection.objects.link(duplicate)
            projectiles.append(duplicate)
        
        # create CacheInstance object
        self._cacheInstance = CacheInstance(projectiles)

class StringInstrument(Instrument):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

        self.preAnimate()

    def preAnimate(self):
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)
