from __future__ import annotations
import bpy
from typing import List, Tuple
from dataclasses import dataclass
from math import floor, ceil
from typing import Dict, List, Tuple, Optional, TYPE_CHECKING

from .. utils.blender import *

if TYPE_CHECKING:
    from .. src.MIDIStructure import MIDITrack

class BlenderKeyFrameBase:
    fCurves: List[bpy.types.FCurve]

    def __init__(self, fCurves: List[bpy.types.FCurve]):
        self.fCurves = fCurves

    def makeKeyFrames(self, object: bpy.types.Object, delta: int, frame: int, locationObject: Optional[bpy.types.object] = None):

        # if there is already a keyframe of this type (such as location), then we need to get the value
        # of the existing keyframe and add the results
        # otherwise evaluate the FCurves and make the keyframe
        raise NotImplementedError()

class BlenderLocationKeyFrame(BlenderKeyFrameBase):
    def makeKeyFrames(self, object: bpy.types.Object, delta: int, frame: int, locationObject: Optional[bpy.types.object] = None):

        # object == ball
        # locationObject == funnel

        # see note in base class if there is already a key frame to add the values

        # get the location of the

        # process the list of fCurves
        # location = [0] * 3
        # offset = [0, 0, 0]
        # if locationObject is not None:
        #     offset = locationObject.location
        # else:
        #     offset = object.location
        #
        # fCurveIndexes = set()
        # for fCurve in self.fCurves:
        #     i = fCurve.array_index
        #     val = fCurve.evaluate(delta)
        #
        #     fCurveIndexes.add(i)
        #
        #     location[i] = val + offset[i]
        #
        # if locationObject is not None:
        #     for locationMissing in {0, 1, 2}.difference(fCurveIndexes):
        #         location[locationMissing] = offset[locationMissing]
        
        if locationObject is None:
            location = object.location.copy()
        else:
            location = locationObject.location.copy()

        for fCurve in self.fCurves:
            i = fCurve.array_index
            val = fCurve.evaluate(delta)
            location[i] = val
        
        object.location = location
        object.keyframe_insert(data_path="location", frame=frame)

class BlenderRotationKeyFrame(BlenderKeyFrameBase):
    def makeKeyFrames(self, object: bpy.types.Object, delta: int, frame: int, locationObject: Optional[bpy.types.object] = None):
        object.keyframe_insert(data_path="rotation", frame=frame)

class BlenderShapeKeyKeyFrame(BlenderKeyFrameBase):
    def makeKeyFrames(self, object: bpy.types.Object, delta: int, frame: int, locationObject: Optional[bpy.types.object] = None):
        object.keyframe_insert(data_path="", frame=frame)

def maxNeeded(intervals: List[FrameRange]) -> int:
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
class Instrument:
    """base class for instruments that are played for notes"""
    collection: bpy.types.Collection
    midiTrack: 'MIDITrack'
    noteToObjTable: Dict[int, bpy.types.Object]
    _keyFrameInfo: List[BlenderKeyFrameBase]
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
        self._keyFrameInfo = []

        self._activeObjectList = []
        self._activeNoteDict = dict()
        self._frameStart = 0
        self._frameEnd = 0
        self._objFrameRanges = []

        self.createNoteToObjTable()
        self._getKeyFrameObjects()

    def _getKeyFrameObjects(self):
        fCurveType = {}
        for obj in self.collection.all_objects:
            # this will be location, rotation, scale
            for fCrv in FCurvesFromObject(obj.animation_curve):
                if fCrv.data_path in fCurveType:
                    fCurveType[fCrv.data_path].append(fCrv)
                else:
                    fCurveType[fCrv.data_path] = [fCrv]

        for curveType in fCurveType:
            keyFrameObj = None
            if curveType == "location":
                keyFrameObj = BlenderLocationKeyFrame(fCurveType[curveType])
            elif curveType == "rotation_euler":
                keyFrameObj = BlenderRotationKeyFrame(fCurveType[curveType])
            # FIXME: handle others
            if keyFrameObj is not None:
                self._keyFrameInfo.append(keyFrameObj)

        # FIXME: iterate over others (shape keys, materials) and make those

    def preAnimate(self):
        pass

    def preFrameLoop(self) -> int:
        # copy the frames to not mutate them, then sort by start time and then reverse
        self.createFrameRanges()
        self.preAnimate()
        self._objFrameRanges.sort(reverse=True)

        # variables
        frameStart, frameEnd = bpy.context.scene.frame_start, bpy.context.scene.frame_end

        # XXX offeset frameStart by the first objectFrame's start time if it's negative
        # XXX discussion question, could frameStart, frameEnd be simplified by using this? we only need to animate the range between
        # XXX objectFrames[-1]._startFrame, objectFrames[0]._endFrame

        objFirstFrame, objLastFrame = self._objFrameRanges[-1].startFrame, self._objFrameRanges[0].endFrame
        # frameStart = objFirstFrame if objFirstFrame < 0 else frameStarts
        self._frameStart, self._frameEnd = objFirstFrame - 1, objLastFrame + 1  # -1 & +1 to make sure they're within bounds

        return self._frameStart

    def postFrameLoop(self):
        self.noteToObjTable = dict()
        self._activeObjectList = []
        self._activeNoteDict = dict()
        self._objFrameRanges = []

    def animateFrames(self):
        for frame in range(self._frameStart, self._frameEnd):
            self.updateActiveObjectList(frame)
            
            # update activeNoteDict: the notes that are still playing

            # add a keyframe for each object that is moving during this frame
            # call animate method to insert a keyframe
            # instead of passing activeObjectList, pass in activeNoteDict
            self.animate(frame)

    def updateActiveObjectList(self, frame: int):
        # variables
        cache = None
        if hasattr(self, '_cacheInstance'):
            cache = self._cacheInstance

        stillActiveList = []
        # delete/return to cache for old objects
        for frameInfo in self._activeObjectList:
            objStartFrame = frameInfo.startFrame
            objEndFrame = frameInfo.endFrame
            obj = frameInfo.obj
            cachedObj = frameInfo.cachedObj

            if objEndFrame >= frame:
                stillActiveList.append(frameInfo)
            else:
                # this note is no longer being played
                # remove from activeNoteDict
                self._activeNoteDict[obj.note_number].remove(frameInfo)

                # RETURN OBJECT TO CACHE
                if cache is not None and cachedObj is not None:
                    cache.pushObject(cachedObj)

                    # disable cached object in viewport/render
                    cachedObj.hide_viewport = True
                    cachedObj.hide_render = True
                    cachedObj.keyframe_insert(data_path="hide_viewport", frame=frame)
                    cachedObj.keyframe_insert(data_path="hide_render", frame=frame)

        self._activeObjectList = stillActiveList

        # update activeObjectList with new objects
        i = len(self._objFrameRanges) - 1

        while i >= 0 and self._objFrameRanges[i].startFrame <= frame:
            frameInfo = self._objFrameRanges[i]
            objStartFrame = frameInfo.startFrame
            # objEndFrame = frameInfo.endFrame
            # cachedObj = frameInfo.cachedObj
            obj = frameInfo.obj
            # insType = self.collection.instrument_type

            if cache is not None:
                # this is of cache type,
                if objStartFrame == frame:
                    # GET OBJECT FROM CACHE
                    cachedObj = cache.popObject()

                    # make last keyframe interpolation constant
                    setInterpolationForLastKeyframe(cachedObj, "CONSTANT")

                    # update cached object in frameIno
                    frameInfo.cachedObj = cachedObj

                    # enable cached object in viewport/render
                    cachedObj.hide_viewport = False
                    cachedObj.hide_render = False
                    cachedObj.keyframe_insert(data_path="hide_viewport", frame=frame)
                    cachedObj.keyframe_insert(data_path="hide_render", frame=frame)

                    self._activeObjectList.append(frameInfo)
            else:
                # if not of cache type, just append the active frames
                self._activeObjectList.append(frameInfo)
                # if objStartFrame == frame:
                #     # Debug markers
                #     bpy.context.scene.timeline_markers.new("debug", frame=frame)

            # add note to activeNoteDict
            if obj.note_number in self._activeNoteDict:
                self._activeNoteDict[obj.note_number].append(self._objFrameRanges[i])
            else:
                self._activeNoteDict[obj.note_number] = [self._objFrameRanges[i]]

            # this note will be played next, so we shouldn't iterate over it again for the next frame
            self._objFrameRanges.pop()
            i -= 1

    def createNoteToObjTable(self) -> None:
        for obj in self.collection.all_objects:
            if obj.note_number is None: raise RuntimeError(f"Object '{obj.name}' has no note number!")
            if int(obj.note_number) in self.noteToObjTable: raise RuntimeError(f"There are two objects in the scene with duplicate note numbers.")
            
            self.noteToObjTable[int(obj.note_number)] = obj

    def createFrameRanges(self):

        assert self.noteToObjTable is not None, "please run createNoteToObjTable first"

        out = []

        for note in self.midiTrack.notes:
            # lookup obj from note number
            obj = self.noteToObjTable[note.noteNumber]

            try:
                hit = obj.note_hit_time
            except Exception as e:
                # FIXME figure out exact error type
                print(e)
                hit = 0

            # FIXME: implement with self._keyframeInfo
            # need to look at all the FCurves and compute startFrame and endFrame for each one
            # keep track of minimum startFrame and maximum endFrame
            # and then append that to out
            rangeVector = FCurvesFromObject(obj.animation_curve)[obj.animation_curve_index].range()
            startFCurve, endFCurve = rangeVector[0], rangeVector[1]

            frame = secToFrames(note.timeOn)
            startFrame = int(floor((startFCurve - hit) + frame))
            endFrame = int(ceil((endFCurve - hit) + frame))

            out.append(FrameRange(startFrame, endFrame, obj))

        self._objFrameRanges = out

    # def applyFCurve(self, obj: bpy.types.Object, frameNumber: int) -> Tuple[float, float, float]:
    #     # TODO: new name: createKeyFrame?
    #     # instead of returning, just make the keyframe with keyframe_insert() and return None
    #     raise NotImplementedError("subclass must override")

    def animate(self, frame: int):
        for noteNumber in self._activeNoteDict:
            for frameInfo in self._activeNoteDict[noteNumber]:
                # variables:
                objStartFrame = frameInfo.startFrame
                objEndFrame = frameInfo.endFrame
                obj = frameInfo.obj  # funnel
                cachedObj = frameInfo.cachedObj  # ball

                # insert keyframe here:
                delta = frame - objStartFrame

                for keyframe in self._keyFrameInfo:
                    if cachedObj is None:
                        keyframe.makeKeyFrames(obj, delta, frame)
                    else:
                        keyframe.makeKeyFrames(cachedObj, delta, frame, obj)

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
        maxNumOfProjectiles = maxNeeded(self._objFrameRanges)

        projectiles = []

        for i in range(maxNumOfProjectiles):
            duplicate = objectCollection.reference_projectile.copy()
            duplicate.name = f"projectile_{hex(id(self.midiTrack))}_{i}"
            # duplicate.hide_viewport = True
            # duplicate.hide_render = True
            # duplicate.keyframe_insert(data_path="hide_viewport", frame=0)
            # duplicate.keyframe_insert(data_path="hide_render", frame=0)

            objectCollection.projectile_collection.objects.link(duplicate)
            projectiles.append(duplicate)
        
        # create CacheInstance object
        self._cacheInstance = CacheInstance(projectiles)

    # def applyFCurve(self, obj: bpy.types.Object, frameNumber: int) -> Tuple[float, float, float]:
    #     fCurves = FCurvesFromObject(obj.animation_curve)
    #     if len(fCurves) != 2: raise(RuntimeError("Please make sure FCurve object only has 2 FCurves!"))
    #     if fCurves[0].data_path != "location": raise RuntimeError("FCurve data path must be location data!")
    #
    #     out = [0] * 3
    #     fCurveIndexes = set()
    #     for fCurve in fCurves:
    #
    #         i = fCurve.array_index
    #         val = fCurve.evaluate(frameNumber)
    #
    #         fCurveIndexes.add(i)
    #
    #         out[i] = val
    #
    #     for el in {0, 1, 2}.difference(fCurveIndexes):
    #         locationMissing = el
    #         break
    #
    #     out[locationMissing] = obj.location[locationMissing]
    #
    #     # TODO: fix offset
    #     # for i in range(len(out)):
    #     #     offset = obj.location[i] - out[i]
    #     #     out[i] += offset
    #
    #     return out

    # def animate(self, frame: int):
    #     for noteNumber in self._activeNoteDict:
    #         for frameInfo in self._activeNoteDict[noteNumber]:
    #             # variables:
    #             objStartFrame = frameInfo.startFrame
    #             objEndFrame = frameInfo.endFrame
    #             obj = frameInfo.obj  # funnel
    #             cachedObj = frameInfo.cachedObj  # ball
    #
    #             # insert keyframe here:
    #             hitTime = obj.note_hit_time  # get hit time from funnel
    #             delta = frame - objStartFrame
    #             # x = obj.location[0]
    #             x, y, z = self.applyFCurve(obj, delta)
    #
    #             # make a keyframe for the object for this frame
    #             cachedObj.location = (x, y, z)
    #             cachedObj.keyframe_insert(data_path="location", frame=frame)
    #             setInterpolationForLastKeyframe(cachedObj, "BEZIER")
    #

class StringInstrument(Instrument):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

        self.preAnimate()

    def preAnimate(self):
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)

    # def applyFCurve(self, obj: bpy.types.Object, frameNumber: int) -> Tuple[float, float, float]:
    #     # eval FCurve
    #     # TODO: VERY TEMP
    #     return FCurvesFromObject(obj.animation_curve)[obj.animation_curve_index].evaluate(frameNumber)
    #     return FCurvesFromObject(obj.animation_curve)[0].evaluate(frameNumber), FCurvesFromObject(obj.animation_curve)[1].evaluate(frameNumber)
    #
    # def animate(self, activeNoteDict: List[FrameRange], frame: int):
    #     # build up temp dictionary sorted by noteNumber
    #     # activeNoteDict = {}
    #
    #     # for frameInfo in activeNoteDict:
    #     #     note_number = frameInfo.obj.note_number
    #     #     if note_number in activeNoteDict:
    #     #         activeNoteDict[note_number].append(frameInfo)
    #     #     else:
    #     #         activeNoteDict[note_number] = [frameInfo]
    #
    #     for noteNumber in activeNoteDict:
    #         x, y, z = 0, 0, 0
    #
    #         obj = None
    #         for frameInfo in activeNoteDict[noteNumber]:
    #             objStartFrame = frameInfo.startFrame
    #             obj = frameInfo.obj
    #
    #             delta = frame - objStartFrame
    #
    #             # newX, newY = self.applyFCurve(obj, delta)
    #             newZ = self.applyFCurve(obj, delta)
    #             # x += newX
    #             # y += newY
    #             z += newZ
    #
    #         if obj is not None:
    #             # obj.rotation_euler[0] = x
    #             # obj.rotation_euler[1] = y
    #
    #             # obj.keyframe_insert(data_path="rotation_euler", index=0, frame=frame)
    #             # obj.keyframe_insert(data_path="rotation_euler", index=1, frame=frame)
    #             obj.location[2] = z
    #             obj.keyframe_insert(data_path="location", index=2, frame=frame)
    #
    #
    #     # for frameInfo in activeObjectList:
    #     #     # variables:
    #     #     objStartFrame = frameInfo.startFrame
    #     #     objEndFrame = frameInfo.endFrame
    #     #     obj = frameInfo.obj  # cube
    #
    #     #     # insert keyframe:
    #     #     delta = frame - objStartFrame
    #
    #     #     x, y = self.positionForFrame(obj, delta)
    #     #     # obj.location[2] = self.positionForFrame(obj, delta)
    #     #     obj.rotation_euler[0] = x
    #     #     obj.rotation_euler[1] = y
    #
    #     #     obj.keyframe_insert(data_path="rotation_euler", index=0, frame=frame)
    #     #     obj.keyframe_insert(data_path="rotation_euler", index=1, frame=frame)
