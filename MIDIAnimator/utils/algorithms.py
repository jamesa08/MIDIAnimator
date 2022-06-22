from __future__ import annotations
import bpy
from typing import List, Tuple
from dataclasses import dataclass
from math import floor, ceil
from typing import Dict, List, Tuple, Optional, Union, TYPE_CHECKING
from mathutils import Vector, Euler

from .. utils.blender import *

if TYPE_CHECKING:
    from .. src.MIDIStructure import MIDITrack

class ObjectFCurves:
    location: Tuple[bpy.types.FCurve]
    rotation: Tuple[bpy.types.FCurve]
    shapeKeys: Tuple[bpy.types.FCurve]
    material: Tuple[bpy.types.FCurve]

    def __init__(self, location = (), rotation = (), shapeKeys = (), material = ()):
        self.location = location
        self.rotation = rotation
        self.shapeKeys = shapeKeys
        self.material = material

class FCurveProcessor:
    obj: bpy.types.Object
    locationObject: Optional[bpy.types.Object]
    fCurves: ObjectFCurves
    location: Optional[Vector]
    rotation: Optional[Euler]
    material: Dict[str, Union[int, float]]

    def __init__(self, obj: bpy.types.Object, fCurves: ObjectFCurves, locationObject: Optional[bpy.types.Object] = None):
        self.obj = obj
        self.fCurves = fCurves
        self.locationObject = locationObject
        # when None no keyframe of that type
        self.location = None
        self.rotation = None
        self.material = {}

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


class BlenderKeyFrameBase:
    fCurves: List[bpy.types.FCurve]

    def __init__(self, fCurves: List[bpy.types.FCurve]):
        self.fCurves = fCurves
        self.keyFrameDict = {}


    def seeIfKeyframeExistsAtFrame(self, data_path, array_index, obj, frame):
        for fCrv in FCurvesFromObject(obj):
            if fCrv.data_path != data_path or fCrv.array_index != array_index: continue
            for keyframe in reversed(fCrv.keyframe_points):
                if keyframe.co[0] == frame:
                    return keyframe.co[1]
            
        return 0

    def updateKeyFrame(self):
        pass

    def makeKeyFrames(self, obj: bpy.types.Object, delta: int, frame: int, locationObject: Optional[bpy.types.object] = None):

        # if there is already a keyframe of this type (such as location), then we need to get the value
        # of the existing keyframe and add the results
        # otherwise evaluate the FCurves and make the keyframe
        raise NotImplementedError()

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
    _keyFrameInfo: Dict[bpy.types.Object, ObjectFCurves]
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
        self._keyFrameInfo = dict()

        self._activeObjectList = []
        self._activeNoteDict = dict()
        self._frameStart = 0
        self._frameEnd = 0
        self._objFrameRanges = []

        self.createNoteToObjTable()
        self._makeObjToFCurveDict()

    def _makeObjToFCurveDict(self):
        for obj in self.collection.all_objects:
            location = []
            rotation = []
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
                # elif dataPath == "shape_key":
                #     shapeKeys.append(fCrv)
            
            self._keyFrameInfo[obj] = ObjectFCurves(tuple(location), tuple(rotation), tuple(shapeKeys), tuple(material))

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

    def animate(self, frame: int):
        # each cached object needs a separate FCurveProcessor since the same note can be "in progress" more than
        # once for a given frame
        cacheObjectProcessors = {}

        # for this note, iterate over all frame ranges for the note being played with objects still moving
        for noteNumber in self._activeNoteDict:

            if len(self._activeNoteDict[noteNumber]) <= 0:
                continue
        
            frameInfo = self._activeNoteDict[noteNumber][0]
            obj = frameInfo.obj  # funnel, doesn't change until outer loop is completed
            objFCurve = self._keyFrameInfo[obj] # get this from our dictionary mapping obj to ObjectFCurves

            processor = FCurveProcessor(obj, objFCurve)

            # set of FCurveProcessor in case more than one cached object currently in-progress for the same note
            processorSet = set()
            for frameInfo in self._activeNoteDict[noteNumber]:
                # check if we are animating a cached object for this
                cachedObj = frameInfo.cachedObj
                if cachedObj is not None:
                    # if not the first time we are using this cachedObject, get its FCurveProcessor
                    if cachedObj in cacheObjectProcessors:
                        processor = cacheObjectProcessors[cachedObj]
                    # this is the first time this cachedObject is used so need to create its FCurveProcessor
                    else:
                        processor = FCurveProcessor(cachedObj, objFCurve, obj)
                        cacheObjectProcessors[cachedObj] = processor
                # make certain processorSet contains all FCurveProcessors being used for this note
                processorSet.add(processor)

                # accumulate the FCurve results for this object
                objStartFrame = frameInfo.startFrame
                delta = frame - objStartFrame
                processor.applyFCurve(delta)

            # for each object (could be multiple cached objects) insert the key frame
            for p in processorSet:
                p.insertKeyFrames(frame)

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
            
            # hide them
            duplicate.hide_viewport = True
            duplicate.hide_render = True
            duplicate.keyframe_insert(data_path="hide_viewport", frame=self._objFrameRanges[0].startFrame)
            duplicate.keyframe_insert(data_path="hide_render", frame=self._objFrameRanges[0].startFrame)

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
