from __future__ import annotations
import bpy
from dataclasses import dataclass
from math import floor, ceil, radians
from typing import Dict, List, Tuple, Optional, Union, TYPE_CHECKING
from mathutils import Vector, Euler
from contextlib import suppress

from .. utils.loggerSetup import *
from .. data_structures import *
from .. utils.blender import *
from . algorithms import *


if TYPE_CHECKING:
    from .. data_structures.midi import *

@dataclass
class Instrument:
    """base class for instruments that are played for notes"""
    collection: bpy.types.Collection
    midiTrack: 'MIDITrack'
    noteToNoteAnimatorTable: Dict[int, bpy.types.Object]
    _activeObjectList: List[FrameRange]
    _activeNoteDict: Dict[int, List[FrameRange]]
    _frameStart: int
    _frameEnd: int
    _objFrameRanges: List[FrameRange]

    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, override=False):
        self.collection = collection
        self.midiTrack = midiTrack
        self.noteToNoteAnimatorTable = dict()
        self.override = override
        self._activeObjectList = []
        self._activeNoteDict = dict()
        self._frameStart = 0
        self._frameEnd = 0
        self._objFrameRanges = []

        if not self.override:
            self.__post_init__()
    
    def __post_init__(self):
        result = self._makeObjToFCurveDict()
        self.createNoteToNoteAnimatorTable(result)
    
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
            for key in shapeKeysDict.copy():  # NOTE: .keys() did not work, same error (dict change size during iteration   )
                val = shapeKeysDict[key]
                if len(val) != 2: 
                    del shapeKeysDict[key]
                    continue
                
                shapeKeys.append(val[0])  # add the FCurve only to the internal list of shapeKeys
            
            fCurveDict[obj] = ObjectFCurves(tuple(location), tuple(rotation), shapeKeysDict, tuple(shapeKeys), tuple(material))

        return fCurveDict

    def createNoteToNoteAnimatorTable(self, fCurveDict: Dict[bpy.types.Object, ObjectFCurves]) -> None:
        for obj in self.collection.all_objects:
            if obj.note_number is None: raise RuntimeError(f"Object '{obj.name}' has no note number!")
            if int(obj.note_number) in self.noteToNoteAnimatorTable: raise RuntimeError(
                f"There are two objects in the scene with duplicate note numbers.")
            na = NoteAnimator(obj, fCurveDict[obj])
            self.noteToNoteAnimatorTable[int(obj.note_number)] = na

    def preAnimate(self):
        pass

    def preFrameLoop(self):
        # copy the frames to not mutate them, then sort by start time and then reverse
        self.createFrameRanges()
        self.preAnimate()
        self._objFrameRanges.sort(reverse=True)
        assert len(self._objFrameRanges) != 0, "There are no object frames! Are there notes assigned to objects?"
        self._frameStart, self._frameEnd = self._objFrameRanges[-1].startFrame, self._objFrameRanges[0].endFrame

    def postFrameLoop(self):
        self.noteToNoteAnimatorTable = dict()
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
                # set it's note number (for use with drivers)
                cachedObj.note_number_int = int(obj.note_number)
                cachedObj.keyframe_insert(data_path="note_number_int", frame=frame+offset)

                # make last keyframe interpolation constant
                setInterpolationForLastKeyframe(cachedObj, "CONSTANT")
                
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
        assert self.noteToNoteAnimatorTable is not None, "please run createNoteToObjTable first"
        result = []

        for note in self.midiTrack.notes:
            # lookup obj from note number
            try:
                noteAnimator = self.noteToNoteAnimatorTable[note.noteNumber]
            except KeyError:
                print(f"Note {note} has no object!")
                continue # ignore note, likely just unused
            obj = noteAnimator.obj

            try:
                hit = obj.note_hit_time
            except AttributeError:
                print(f"{obj.name} has no hit time!")
                hit = 0

            frame = int(secToFrames(note.timeOn))
            offsets = noteAnimator.frameOffsets()
            startFrame = int(floor(offsets[0] - hit + frame)) - 1
            endFrame = int(ceil(offsets[1] - hit + frame)) + 1
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
            objFCurve = self.noteToNoteAnimatorTable[int(obj.note_number)].animators
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
            duplicate.name = f"projectile_{hex(id(objectCollection))}_{i}"
            
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
        bpy.context.scene.frame_set(-10000)
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)