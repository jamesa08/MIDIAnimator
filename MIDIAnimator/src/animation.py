from __future__ import annotations
from typing import List, Dict
import bpy

from .. data_structures.midi import MIDITrack
from .. utils.loggerSetup import *
from .. src.instruments import *
from . algorithms import *

class MIDIAnimatorNode:
    """This class encompasses all `Instrument` classes (and its subclasses)."""
    _instruments: List[Instrument]

    def __init__(self):
        self._instruments = []

    def addInstrument(self, instrumentType: str, midiTrack: MIDITrack, objectCollection: bpy.types.Collection, properties: Dict[str]=None, custom=None, customVars: Dict=None):
        """adds an instrument to MIDIAnimator. This will create the class for you

        :param str instrumentType: The instrument type. Choose from "evaluate", "Projectile" or "Custom". 
        :param MIDITrack midiTrack: The `MIDITrack` object to create the instrument from
        :param bpy.types.Collection objectCollection: The collection (`bpy.types.Collection`) of Blender objects to be animated.
        :param Dict[str] properties: dictionary of properties for classes, defaults to None
        :param class(Instrument) custom: a custom Instrument class that inherits from Instrument, defaults to None
        :param Dict customVars: a dictionary of custom vars you would like to send to your custom class, defaults to None
        :raises RuntimeError: if instrumentType="custom" and the customClass is None.
        """
        assert type(midiTrack).__name__ == "MIDITrack", "Please pass in a type MIDITrack object."
        assert isinstance(objectCollection, bpy.types.Collection), "Please pass in a type collection for the objects to be animated."
        assert instrumentType in {"evaluate", "projectile", "custom"}, "Instrument type invalid."
        
        if instrumentType == "projectile":
            assert properties is not None, "Please pass a dictionary of properties in. Refer to the docs for help."
            assert "projectile_collection" in properties, "Please pass 'projectile_collection' into the properties dictionary."
            assert "reference_projectile" in properties, "Please pass 'reference_projectile' into the properties dictionary."
            
            assert isinstance(properties["projectile_collection"], bpy.types.Collection), "Please pass in a bpy.types.Collection for property 'projectile_collection'."
            assert isinstance(properties["reference_projectile"], bpy.types.Object), "Please pass in a bpy.types.Object for property 'reference_projectile'."
            
            cls = ProjectileInstrument(midiTrack=midiTrack, objectCollection=objectCollection, projectileCollection=properties["projectile_collection"], referenceProjectile=properties["reference_projectile"])
        
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
            instrument.animate()