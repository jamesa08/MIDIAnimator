from __future__ import annotations
from typing import List, Dict
import bpy

from .. data_structures.midi import MIDITrack
from ..utils.logger import logger, buffer
from .. src.instruments import *
from . algorithms import *

class MIDIAnimatorNode:
    """This class encompasses all `Instrument` classes (and its subclasses)."""
    _instruments: List[Instrument]

    def __init__(self):
        self._instruments = []

    def addInstrument(self, instrumentType: str=None, midiTrack: MIDITrack=None, objectCollection: bpy.types.Collection=None, custom=None, customVars: Dict=None):
        """adds an instrument to MIDIAnimator. This will create the class for you

        :param MIDITrack midiTrack: The `MIDITrack` object to create the instrument from
        :param bpy.types.Collection objectCollection: The collection (`bpy.types.Collection`) of Blender objects to be animated.
        :param class(Instrument) custom: a custom Instrument class that inherits from Instrument, defaults to None
        :param Dict customVars: a dictionary of custom vars you would like to send to your custom class, defaults to None
        :raises RuntimeError: if instrumentType="custom" and the customClass is None.
        """
        
        assert type(midiTrack).__name__ == "MIDITrack", "Please pass in a type MIDITrack object."
        assert isinstance(objectCollection, bpy.types.Collection), "Please pass in a type collection for the objects to be animated."

        if instrumentType:
            logger.warn("The `instrumentType` parameter has been depercated and is no longer required.")
            objectCollection.midi.instrument_type = instrumentType

        instrumentType = objectCollection.midi.instrument_type
        
        try: 
            if instrumentType == "custom":
                if custom is None: raise RuntimeError("Please pass a custom class object. Refer to the docs for help.")
                if customVars is not None: 
                    cls = custom(midiTrack, objectCollection, **customVars)
                else:
                    cls = custom(midiTrack, objectCollection)
            else:
                for item in Instruments:
                    value = item.value
                    if value.identifier == instrumentType:
                        instrumentCls = value.cls
                        break
                
                cls = instrumentCls(midiTrack, objectCollection)
        except Exception as e:
            logger.exception(f"Error while creating instrument: '{e}'")
            raise e
        

        self._instruments.append(cls)

    def animate(self) -> None:
        """animate all of the tracks"""
        try: 
            for instrument in self._instruments:
                instrument.animate()
        except Exception as e:
            logger.exception(f"Error while animating instrument: {e}")
            raise e