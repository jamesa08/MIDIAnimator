from __future__ import annotations
import bpy
from typing import List, Dict

from . algorithms import *
from .. utils.loggerSetup import *
from .. src.instruments import *
from .. data_structures.midi import MIDITrack

class MIDIAnimatorNode:
    """this class acts as a wrapper for GenericTracks/custom tracks"""
    _instruments: List[Instrument]

    def __init__(self):
        self._instruments = []

    def addInstrument(self, instrumentType: str, midiTrack: MIDITrack, objectCollection: bpy.types.Collection, properties=None, custom=None, customVars: Dict=None):
        assert isinstance(midiTrack, MIDITrack), "Please pass in a type MIDITrack object."
        assert isinstance(objectCollection, bpy.types.Collection), "Please pass in a type collection for the objects to be animated."
        assert instrumentType in {"evaluate", "projectile", "custom"}, "Instrument type invalid."
        
        if instrumentType == "projectile":
            assert properties is not None, "Please pass a dictionary of properties in. Refer to the docs for help."
            assert "projectile_collection" in properties, "Please pass 'projectile_collection' into the properties dictionary."
            assert "reference_projectile" in properties, "Please pass 'reference_projectile' into the properties dictionary."
            
            assert isinstance(properties["projectile_collection"], bpy.types.Collection), "Please pass in a bpy.types.Collection for property 'projectile_collection'."
            assert isinstance(properties["reference_projectile"], bpy.types.Object), "Please pass in a bpy.types.Object for property 'reference_projectile'."
            
            cls = ProjectileInstrument(midiTrack, objectCollection, properties["projectile_collection"], properties["reference_projectile"])
        
        elif instrumentType == "evaluate": 
            cls = EvaluateInstrument(midiTrack, objectCollection)

        elif instrumentType == "custom":
            if custom is None: raise RuntimeError("Please pass a custom class object. Refer to the docs for help.")
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