from __future__ import annotations
import bpy
from typing import List, Tuple
from dataclasses import dataclass
from math import sin, cos, floor, ceil
from typing import Dict, List, Tuple, Optional, TYPE_CHECKING

from .. utils.blender import *

if TYPE_CHECKING:
    from .. src.MIDIStructure import MIDITrack

def maxNeeded(intervals: FrameRange) -> int:
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
        endIndex  = 0
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

def rotateAroundCircle(angle, radius) -> Tuple[int]:
    x = sin(angle) * radius
    y = cos(angle) * radius
    
    return x, y

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
    # data structure similar to a stack implementation
    _cache = List[bpy.types.Object]

    def __init__(self, objs: List[bpy.types.Object]) -> None:
        """intializes a type List[bpy.types.Object] to the cache instance."""
        self._cache = objs

    def pushObject(self, obj: bpy.types.Object) -> None:
        """pushes a bpy.types.Object back to the cache. This is the method to use when you are done with the object"""
        # self._cache[objType].append(obj)
        self._cache.append(obj)
    
    def popObject(self) -> bpy.types.Object:
        """removes a bpy.types.Object from the cache and returns it. This is the method to use when you want to take an object out"""
        # obj = self._cache[objType].pop()
        obj = self._cache.pop()
        return obj

@dataclass
class GenericInstrument: 
    """base class for instruments that are played for notes"""
    _collection = bpy.types.Collection
    _midiTrack: 'MIDITrack'
    _noteToObjTable: Dict[int, bpy.types.Object]

    def __init__(self, midiTrack: 'MIDITrack', collection: bpy.types.Collection):
        self._collection = collection
        self._midiTrack = midiTrack
        self._noteToObjTable = dict()

        self.createNoteToObjTable()

    def preAnimate(self):
        raise NotImplementedError("subclass must override")

    def createNoteToObjTable(self) -> None:
        for obj in self._collection.all_objects:
            if obj.note_number is None: raise RuntimeError(f"Object '{obj.name}' has no note number!")
            if int(obj.note_number) in self._noteToObjTable: raise RuntimeError(f"There are two objects in the scene with duplicate note numbers.")
            
            self._noteToObjTable[int(obj.note_number)] = obj

    def createFrameRanges(self) -> FrameRange:
        raise NotImplementedError("subclass must override")

    def applyFCurve(self, obj: bpy.types.Object, frameNumber: int) -> Tuple[float, float, float]:
        # TODO: new name: createKeyFrame?
        # instead of returning, just make the keyframe with keyframe_insert() and return None
        raise NotImplementedError("subclass must override")

    def animate(self, activeObjectList: List[FrameRange], frame: int):
        raise NotImplementedError("subclass must override")


class ProjectileInstrument(GenericInstrument):
    _cacheInstance: CacheInstance

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

        # post init things here
        self._cacheInstance = None
        self.preAnimate()
    
    def preAnimate(self):
        # delete old objects
        objectCollection = self._collection

        assert objectCollection.projectile_collection is not None, "Please define a Projectile collection for type Projectile."
        assert objectCollection.reference_projectile is not None, "Please define a reference object to be duplicated."
        cleanCollection(objectCollection.projectile_collection, objectCollection.reference_projectile)

        # calculate number of needed projectiles & instance the blender objects using bpy
        maxNumOfProjectiles = maxNeeded(self.createFrameRanges())  

        projectiles = []

        for i in range(maxNumOfProjectiles):
            duplicate = objectCollection.reference_projectile.copy()
            duplicate.name = f"projectile_{hex(id(self._midiTrack))}_{i}"
            objectCollection.projectile_collection.objects.link(duplicate)
            projectiles.append(duplicate)
        
        # create CacheInstance object
        self._cacheInstance = CacheInstance(projectiles)

    def createFrameRanges(self) -> List[FrameRange]:
        assert self._noteToObjTable is not None, "please run createNoteToObjTable first"
        
        out = []
        
        for note in self._midiTrack.notes:
            # lookup obj from note number
            obj = self._noteToObjTable[note.noteNumber]

            hit = obj.note_hit_time
            
            rangeVector = FCurvesFromObject(obj.animation_curve)[obj.animation_curve_index].range()
            startFCurve, endFCurve = rangeVector[0], rangeVector[1]
            
            frame = secToFrames(note.timeOn)
            startFrame = int(floor((startFCurve - hit) + frame))
            endFrame = int(ceil((endFCurve - hit) + frame))
            
            out.append(FrameRange(startFrame, endFrame, obj))
        
        return out

    def applyFCurve(self, obj: bpy.types.Object, frameNumber: int) -> Tuple[float, float, float]:
        # new name -> applyFCurve
        fCurves = FCurvesFromObject(obj.animation_curve)
        if len(fCurves) != 2: raise(RuntimeError("Please make sure FCurve object only has 2 FCurves!"))
        if fCurves[0].data_path != "location": raise RuntimeError("FCurve data path must be location data!")
        
        out = [0] * 3
        fCurveIndexes = set()
        for fCurve in fCurves:
            
            i = fCurve.array_index
            val = fCurve.evaluate(frameNumber)
            
            fCurveIndexes.add(i)
            
            out[i] = val
        
        for el in {0, 1, 2}.difference(fCurveIndexes):
            locationMissing = el
            break
        
        out[locationMissing] = obj.location[locationMissing]
        
        # TODO: fix offset
        # for i in range(len(out)):
        #     offset = obj.location[i] - out[i]
        #     out[i] += offset
    
        return out

    def animate(self, activeNoteDict: List[FrameRange], frame: int):
        for noteNumber in activeNoteDict:
            for frameInfo in activeNoteDict[noteNumber]:
                # variables:
                objStartFrame = frameInfo.startFrame
                objEndFrame = frameInfo.endFrame
                obj = frameInfo.obj  # funnel
                cachedObj = frameInfo.cachedObj  # ball

                # insert keyframe here:
                hitTime = obj.note_hit_time  # get hit time from funnel
                delta = frame - objStartFrame
                # x = obj.location[0]
                x, y, z = self.applyFCurve(obj, delta)

                # make a keyframe for the object for this frame
                cachedObj.location = (x, y, z)
                cachedObj.keyframe_insert(data_path="location", frame=frame)
                setInterpolationForLastKeyframe(cachedObj, "BEZIER")
                    

class StringInstrument(GenericInstrument):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

        self.preAnimate()

    def preAnimate(self):
        for obj in self._collection.all_objects:
            cleanKeyframes(obj)

    def createFrameRanges(self) -> FrameRange:
        assert self._noteToObjTable is not None, "please run createNoteToObjTable first"
        out = []
        for note in self._midiTrack.notes:
            # lookup obj from note number
            obj = self._noteToObjTable[note.noteNumber]

            rangeVector = FCurvesFromObject(obj.animation_curve)[obj.animation_curve_index].range()
            startFCurve, endFCurve = rangeVector[0], rangeVector[1]
            
            frame = secToFrames(note.timeOn)
            startFrame = round(int(startFCurve + frame))
            endFrame = round(int(endFCurve + frame))
            
            out.append(FrameRange(startFrame, endFrame, obj))
        
        return out

    def applyFCurve(self, obj: bpy.types.Object, frameNumber: int) -> Tuple[float, float, float]:
        # eval FCurve
        # TODO: VERY TEMP
        return FCurvesFromObject(obj.animation_curve)[0].evaluate(frameNumber), FCurvesFromObject(obj.animation_curve)[1].evaluate(frameNumber)
    
    def animate(self, activeNoteDict: List[FrameRange], frame: int):
        # build up temp dictionary sorted by noteNumber
        # activeNoteDict = {}

        # for frameInfo in activeNoteDict:
        #     note_number = frameInfo.obj.note_number
        #     if note_number in activeNoteDict:
        #         activeNoteDict[note_number].append(frameInfo)
        #     else:
        #         activeNoteDict[note_number] = [frameInfo]
        
        for noteNumber in activeNoteDict:
            x, y = 0, 0

            obj = None
            for frameInfo in activeNoteDict[noteNumber]:
                objStartFrame = frameInfo.startFrame
                obj = frameInfo.obj
                
                delta = frame - objStartFrame
                
                newX, newY = self.applyFCurve(obj, delta)
                x += newX
                y += newY
            
            if obj is not None:
                obj.rotation_euler[0] = x
                obj.rotation_euler[1] = y
            
                obj.keyframe_insert(data_path="rotation_euler", index=0, frame=frame)
                obj.keyframe_insert(data_path="rotation_euler", index=1, frame=frame)


        # for frameInfo in activeObjectList:
        #     # variables: 
        #     objStartFrame = frameInfo.startFrame
        #     objEndFrame = frameInfo.endFrame
        #     obj = frameInfo.obj  # cube

        #     # insert keyframe:
        #     delta = frame - objStartFrame
            
        #     x, y = self.positionForFrame(obj, delta)
        #     # obj.location[2] = self.positionForFrame(obj, delta)
        #     obj.rotation_euler[0] = x
        #     obj.rotation_euler[1] = y
        
        #     obj.keyframe_insert(data_path="rotation_euler", index=0, frame=frame)
        #     obj.keyframe_insert(data_path="rotation_euler", index=1, frame=frame)
