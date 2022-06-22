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
    from algorithms import *

@dataclass
class AnimationProperties: # was ObjectFCurves
    location: Tuple[bpy.types.FCurve]
    rotation: Tuple[bpy.types.FCurve]
    shapeKeys: Tuple[bpy.types.FCurve]
    material: Tuple[bpy.types.FCurve]

# noteToObjTable now maps to this


# FrameRange will now have a reference to this
class AnimatingNote:
    _note: int # may not need this
    _startFrame: int
    _endFrame: int
    _hitTime: int
    _obj: Optional[bpy.types.Object] # will only be optional if getting from cache and haven't gotten yet
    _animators: AnimationProperties
    _locationObject: Optional[bpy.types.Object]
    _cache: Optional[CacheInstance]

    _location: Optional[Vector]
    _rotation: Optional[Euler]
    _material: dict[str, Union[int, float]]

    def applyFCurve(self, delta: int):
        # see FCurveProcessor
        pass

    def insertKeyFrames(self, frame: int):
        # see FCurveProcessor
        pass

class Instrument2:
    """base class for instruments that are played for notes"""
    collection: bpy.types.Collection
    midiTrack: 'MIDITrack'
    noteToObjTable: Dict[int, NoteAnimator]
    _keyFrameInfo: Dict[bpy.types.Object, AnimationProperties]
    _activeObjectList: List[FrameRange]
    _activeNoteDict: Dict[int, List[FrameRange]]
    _frameStart: int
    _frameEnd: int
    _objFrameRanges: List[FrameRange]

    def __init__(self):
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

        self._makeObjToFCurveDict()
        self._createNoteTable()

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
            properties = AnimationProperties(tuple(location), tuple(rotation), tuple(shapeKeys), tuple(material))
            self._keyFrameInfo[obj] = properties


    def _createNoteTable(self) -> None:
        for obj in self.collection.all_objects:
            if obj.note_number is None:
                raise RuntimeError(f"Object '{obj.name}' has no note number!")
            if int(obj.note_number) in self.noteToObjTable:
                raise RuntimeError(f"There are two objects in the scene with duplicate note numbers.")

            na = NoteAnimator(obj, self._keyFrameInfo[obj])
            self.noteToObjTable[int(obj.note_number)] = na

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

        stillActiveList = []
        for frameInfo in self._activeObjectList:
            endFrame = frameInfo.endFrame
            animatingNote: AnimatingNote = frameInfo.animatingNote
            noteNumber = animatingNote._obj.noteNumber
            obj = animatingNote._obj

            if endFrame >= frame:
                stillActiveList.append(frameInfo)
            else:
                # no longer being played
                self._activeNoteDict[noteNumber].remove(frameInfo)
                cache = animatingNote._cache
                # if it is a cached object, return to the cache
                if cache is not None:
                    cache.returnObject(obj)
                    # disable cached object in viewport/render
                    obj.hide_viewport = True
                    obj.hide_render = True
                    obj.keyframe_insert(data_path="hide_viewport", frame=frame)
                    obj.keyframe_insert(data_path="hide_render", frame=frame)

        self._activeObjectList = stillActiveList
        # update activeObjectList with new objects
        i = len(self._objFrameRanges) - 1

        while i >= 0 and self._objFrameRanges[i].startFrame <= frame:
            frameInfo = self._objFrameRanges[i]
            objStartFrame = frameInfo.startFrame


    def createFrameRanges(self):
        assert self.noteToObjTable is not None, "please run createNoteToObjTable first"
        result = []
        for note in self.midiTrack.notes:
            # lookup obj from note number
            na = self.noteToObjTable[note.noteNumber]

            try:
                hit = na.obj.note_hit_time
            except Exception as e:
                # FIXME figure out exact error type
                print(e)
                hit = 0

            frame = secToFrames(note.timeOn)
            offsets = na.frameOffsets()
            # FIXME change this to + offsets[0] if it's a negative value
            startFrame = frame - offsets[0]
            endFrame = frame + offsets[1]

            # FIXME - determine what to pass to __init__
            animatingNote = AnimatingNote(na.obj, na.animators, startFrame, endFrame)
            result.append(FrameRange(startFrame, endFrame, animatingNote))

        self._objFrameRanges = result

    def animate(self, frame: int):

        for noteNumber in self._activeNoteDict:

            for frameInfo in self._activeNoteDict[noteNumber]
































