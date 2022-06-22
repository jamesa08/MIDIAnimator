from __future__ import annotations
import bpy
from contextlib import suppress
from dataclasses import dataclass
from typing import Callable, List, Tuple, Dict, Optional, Union

from .. utils.algorithms import *
from .. utils.functions import noteToName
from . MIDIStructure import MIDIFile, MIDITrack, MIDINote
from .. utils.blender import FCurvesFromObject, deleteMarkers, secToFrames, cleanCollection, setInterpolationForLastKeyframe

class BlenderAnimation:
    """this class acts as a wrapper for GenericTracks/custom tracks"""
    _instruments: List[Instrument]

    def __init__(self):
        self._instruments = []

    def addInstrument(self, midiTrack: MIDITrack, objectCollection: bpy.types.Collection, custom=None, customVars: Dict=dict()):
        """ make a GenericInstrumnet subclass and add it into internal track list 
            cache only supported for type `projectile` instruments
        """
        assert isinstance(midiTrack, MIDITrack), "Please pass in a type MIDITrack object."
        assert isinstance(objectCollection, bpy.types.Collection), "Please pass in a type collection for the objects to be animated."
        
        insType = objectCollection.instrument_type
        
        if insType == "projectile": cls = ProjectileInstrument(midiTrack, objectCollection)
        elif insType == "string": cls = StringInstrument(midiTrack, objectCollection)
        elif insType == "custom":
            if custom is None: raise RuntimeError("Please pass a custom class object. Refer to the docs.")
            if customVars is not None: 
                cls = custom(midiTrack, objectCollection, **customVars)
            else:
                cls = custom(midiTrack, objectCollection)
        
        self._instruments.append(cls)

    def animate(self, offset: int = 0) -> None:
        """animate all of the tracks

        :param int offset: amount to add to the frame number for each keyframe (in case we have negative keyframes), defaults to 0
        """
        for instrument in self._instruments:
            if instrument.override:
                instrument.animate()
                continue

            instrument.preFrameLoop()
            instrument.animateFrames(offset)
            instrument.postFrameLoop()