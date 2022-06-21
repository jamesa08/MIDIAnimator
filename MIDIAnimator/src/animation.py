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
    _tracks: List[Instrument]

    def __init__(self):
        self._tracks = []
        self._activeNoteDict = dict()

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
        
        self._tracks.append(cls)


    def animate(self) -> None:
        # FIXME: delete all keyframes from frameStart, frameEnd = bpy.context.scene.frame_start, bpy.context.scene.frame_end

        for track in self._tracks:
            if track.override:
                track.animate()
                continue

            track.preFrameLoop()
            track.animateFrames()
            track.postFrameLoop()