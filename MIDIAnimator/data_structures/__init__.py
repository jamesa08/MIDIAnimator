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
    """A simple keyframe data structure.
    :param float frame: the frame value of the keyframe (x)
    :param float value: the value of the keyframe (y)
    """
    frame: float  # frame of the keyframe
    value: float  # value of the keyframe

    @property
    def co(self):
        return (self.frame, self.value)

    def __hash__(self) -> int:
        return hash((self.frame, self.value))

@dataclass
class KeyframeSeconds:
    """A simple keyframe data structure for time in seconds.
    :param float seconds: the seconds value of the keyframe (x)
    :param float value: the value of the keyframe (y)
    """
    seconds: float  # seconds of the keyframe
    value: float  # value of the keyframe

    @property
    def co(self):
        return (self.seconds, self.value)

    def __hash__(self) -> int:
        return hash((self.seconds, self.value))


@dataclass
class DummyFCurve:
    keyframe_points: Tuple[Keyframe]
    array_index: int
    data_path: str
    
    @staticmethod
    def range():
        return 0, 1
    
    def evaluate(self, frame):
        return self.keyframe_points[0].value

@dataclass(init=False)
class ObjectWrapper:
    obj: bpy.types.Object
    noteNumbers: Tuple[int]
    
    noteOnCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]]
    noteOffCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]]

    # these are for all the animation FCurves assigned to the object (beginning frame, ending frame)
    startFrame: float
    endFrame: float

    def __init__(self, obj: bpy.types.Object, noteNumbers: Tuple[int], noteOnCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]], noteOffCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]]):
        """object wrapper for `bpy.types.Object` objects.
        this allows us to store data with the blender objects (such as FCurve data, note numbers, MIDI information, etc)

        :param bpy.types.Object obj: Blender object
        :param Tuple[int] noteNumbers: a tuple of note numbers of type `int`.
        :param List[Union[bpy.types.FCurve, ObjectShapeKey]] noteOnCurves: A list of NoteOn curves dervided from `obj`.
        :param List[Union[bpy.types.FCurve, ObjectShapeKey]] noteOffCurves: A list of NoteOff curves dervided from `obj`.
        """
        self.obj = obj
        
        self.noteNumbers = noteNumbers
        
        self.noteOnCurves = sorted(noteOnCurves, key=lambda x: f"{x.data_path}_{x.array_index}")
        self.noteOffCurves = sorted(noteOffCurves, key=lambda x: f"{x.data_path}_{x.array_index}")
        
        self.startFrame, self.endFrame = None, None

        # make sure object does not use itself as a note on/off curve for the keyframed type
        # make sure obj has at least a note on/off curve for the keyframed type
        # make sure the FCurves are valid
        # if obj.midi.anim_type == "keyframed":
        #     assert obj.midi.note_on_curve != obj and obj.midi.note_off_curve != obj, f"Object '{obj.name}' cannot use itself as an animation curve (Note On/Off)!"
        #     assert obj.midi.note_on_curve is not None or obj.midi.note_off_curve is not None, f"Object '{obj.name}' does not have a Note On/Off animation curve! To use the Keyframed Animation type, you need to have a Note On curve or a Note Off curve."
        #     assert validateFCurves(noteOnCurves, noteOffCurves) is not False, "Note On FCurve object and the Note Off FCurve object have the different data paths (or extraneous data paths)! Make sure to match the Note On and Note Off data paths."
        self._calculateOffsets()
                
        # these need to be copied at the first keyframe
        self.initalLoc = obj.location.copy()
        self.initalRot = obj.rotation_euler.copy()
        self.initalScl = obj.scale.copy()
    
    def _calculateOffsets(self):
        """mutating function that calculates the range of the Note On FCurves (how long each FCurve on the object will be activelty moving)
        
        :return: None
        """

        for noteOnCurve in self.noteOnCurves:
            curveStart, curveEnd = noteOnCurve.range()
            if self.startFrame is None or curveStart < self.startFrame:
                self.startFrame = curveStart
            if self.endFrame is None or curveEnd > self.endFrame:
                self.endFrame = curveEnd

    def __hash__(self):
        return hash(repr(self.obj.id_data))

@dataclass
class ObjectShapeKey:
    """Wrapper for a `bpy.types.Object`'s Shape Key
    
    :param str name: name of the shape key
    :param bpy.types.FCurve referenceCurve: the reference shape key (with the reference/animation curve)
    :param bpy.types.ShapeKey targetKey: the target shape key, this will be keyframed
    :param str data_path: data path for the shape key (needed for keyframing)
    :param int array_index: array index for the shape key (needed for keyframing)
    """
    name: str = ""
    referenceCurve: bpy.types.FCurve = None
    targetKey: bpy.types.ShapeKey = None

    data_path: str = referenceCurve.data_path if referenceCurve is not None else ""
    array_index: int = 0

    def __hash__(self) -> int:
        return hash((self.referenceCurve, self.targetKey))
    
    def range(self) -> Tuple[float, float]:
        """returns the range (total length) of the reference curve

        :return Tuple[float, float]: start time and end time
        """
        return self.referenceCurve.range()
    
    @property
    def keyframe_points(self):
        return self.referenceCurve.keyframe_points


@dataclass
class FrameRange:
    """
    this stores an object that will be moving from startFrame to endFrame
    """
    startFrame: int
    endFrame: int
    wpr: ObjectWrapper

    def __lt__(self, other: FrameRange) -> FrameRange:
        return self.startFrame < other.startFrame

@dataclass(init=False)
class CacheInstance:
    """CacheInstance handles caching objects by finding avaialbe frame times. If there are not any avaiable frame times, it will create a new object"""
    _cache: Dict[int, List[FrameRange]]

    def __init__(self):
        self._cache = {}
    
    def findRange(self, frameRange: FrameRange):
        """finds a index for the given frameRange

        :param FrameRange frameRange: the FrameRange to insert
        :return int: the index key at which the FrameRange will be inserted into self._cache
        """
        for cacheIndex in self._cache:
            cacheRanges = self._cache[cacheIndex]
            
            latestRange = cacheRanges[-1]
            if latestRange.endFrame > frameRange.startFrame:
                # this spot is already used, continue on with the loop
                continue
            else:
                # this spot is available and can be used 
                return cacheIndex
        
        
        # if we haven't exited with return, then we need to add a new object (addObject will append it to the list)
        index = len(self._cache)
        self._cache[index] = []
        
        return index
    
    def addObject(self, frameRange: FrameRange):
        """adds FrameRange to the cache."""
        indexToAdd = self.findRange(frameRange)
        self._cache[indexToAdd].append(frameRange)

    def getCache(self):
        """gets the entire cache

        :return Dict[int, List[FrameRange]]: the cache instance dictionary with all of the indicies and `FrameRange`s
        """
        return self._cache

    def getStartTime(self):
        """gets the first start time in the cache.
        If there isn't any data yet, it will be 0.

        :return float: the first start time
        """
        if len(self._cache) == 0:
            return 0
        
        # this gets the key of 0 of the first element of the list
        # this doesnt need to be computed every time.. is there a way around this?
        return self._cache[0][0].startFrame