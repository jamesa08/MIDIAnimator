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
class ObjectWrapper:
    """object wrapper for a Blender Object
    this allows us to store data with the blender objects (such as FCurve data, note numbers, MIDI information, etc)
    """

    obj: bpy.types.Object
    noteNumbers: Tuple[int]
    
    noteOnCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]]
    noteOffCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]]

    # these are for all the animation FCurves assigned to the object (beginning frame, ending frame)
    startFrame: float
    endFrame: float

    def __init__(self, obj: bpy.types.Object, noteNumbers: tuple, noteOnCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]], noteOffCurves: List[Union[bpy.types.FCurve, ObjectShapeKey]]):
        self.obj = obj
        
        self.noteNumbers = noteNumbers
        
        self.noteOnCurves = sorted(noteOnCurves, key=lambda x: f"{x.data_path}_{x.array_index}")
        self.noteOffCurves = sorted(noteOffCurves, key=lambda x: f"{x.data_path}_{x.array_index}")
        
        self.startFrame, self.endFrame = None, None

        # make sure object does not use itself as a note on/off curve for the keyframed type
        # make sure obj has at least a note on/off curve for the keyframed type
        # make sure the FCurves are valid
        if obj.midi.anim_type == "keyframed":
            assert obj.midi.note_on_curve != obj and obj.midi.note_off_curve != obj, f"Object '{obj.name}' cannot use itself as an animation curve (Note On/Off)!"
            assert obj.midi.note_on_curve is not None or obj.midi.note_off_curve is not None, f"Object '{obj.name}' does not have a Note On/Off animation curve! To use the Keyframed Animation type, you need to have a Note On curve or a Note Off curve."
            assert validateFCurves(noteOnCurves, noteOffCurves) is not False, "Note On FCurve object and the Note Off FCurve object have the different data paths (or extraneous data paths)! Make sure to match the Note On and Note Off data paths."
            self._calculateOffsets()
                
        # these need to be copied at the first keyframe
        self.initalLoc = obj.location.copy()
        self.initalRot = obj.rotation_euler.copy()
        self.initalScl = obj.scale.copy()
    
    def _calculateOffsets(self):
        """mutating function that calculates the range of the Note On FCurves (how long each FCurve on the object will be activelty moving)"""

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
    name: str = ""
    referenceCurve: bpy.types.FCurve = None
    targetKey: bpy.types.ShapeKey = None

    data_path: str = referenceCurve.data_path if referenceCurve is not None else ""
    array_index: int = 0

    def __hash__(self) -> int:
        return hash((self.referenceCurve, self.targetKey))
    
    def range(self):
        return self.referenceCurve.range()

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
        return self._cache

    def getStartTime(self):
        if len(self._cache) == 0:
            return 0
        
        # this gets the key of 0 of the first element of the list
        # this doesnt need to be computed every time.. is there a way around this?
        return self._cache[0][0].startFrame