from dataclasses import dataclass
from MIDIAnimator.src.animation import *
from MIDIAnimator.utils.blender import *
from MIDIAnimator.utils import *
from typing import Callable, List, Tuple, Dict, Optional, Union
from MIDIAnimator.data_structures.midi import MIDIFile
from math import radians 
import bpy

@dataclass
class KeyframeValue:
    seconds: float
    value: float


class VibeUpDown(Instrument):
    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, **kwargs):
        super().__init__(midiTrack, collection, override=True)
        self.gapLength = kwargs["gap_length"]
        self.animLength = kwargs["anim_length"]
        self.controllers = [kwargs["pp1_controller"], kwargs["pp2_controller"], kwargs["pp3_controller"]]
        self.keyframes = []
        self.vibePositions = {
            # "anim", (pp1, pp2, pp3)
            "up": (0, 0, 0),
            "down": (-58, 31, -63),
            "half": (-58/2, 31/2, -63/2)
        }
        self.preAnimate()
    
    def preAnimate(self):
        bpy.context.scene.frame_set(-10000)
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)

    def animate(self):
        # always start down
        self.keyframes.append(KeyframeValue(seconds=0, value=self.vibePositions["down"]))
        
        # insert down -> up at the beginning of the notes
        self.keyframes.append(KeyframeValue(seconds=self.midiTrack.notes[0].timeOn - self.animLength, value=self.vibePositions["down"]))
        self.keyframes.append(KeyframeValue(seconds=self.midiTrack.notes[0].timeOn, value=self.vibePositions["up"]))

        # insert up -> down on last note
        self.keyframes.append(KeyframeValue(seconds=self.midiTrack.notes[-1].timeOn, value=self.vibePositions["up"]))
        self.keyframes.append(KeyframeValue(seconds=self.midiTrack.notes[-1].timeOn + self.animLength, value=self.vibePositions["down"]))
        

        for i, curNote in enumerate(self.midiTrack.notes):
            nextNote = self.midiTrack.notes[i+1] if i < len(self.midiTrack.notes)-1 else curNote
            noteLength = nextNote.timeOn - curNote.timeOn
            if noteLength >= self.gapLength:
                # insert up -> half on note
                self.keyframes.append(KeyframeValue(seconds=curNote.timeOn, value=self.vibePositions["up"]))
                self.keyframes.append(KeyframeValue(seconds=curNote.timeOn + self.animLength, value=self.vibePositions["half"]))
                
                # insert half -> up when break done
                self.keyframes.append(KeyframeValue(seconds=nextNote.timeOn - self.animLength, value=self.vibePositions["half"]))
                self.keyframes.append(KeyframeValue(seconds=nextNote.timeOn, value=self.vibePositions["up"]))
        

        for keyframe in self.keyframes:
            seconds = keyframe.seconds
            values = keyframe.value

            for value, controller in zip(values, self.controllers):
                controller.rotation_euler.y = radians(value)
                controller.keyframe_insert(data_path="rotation_euler", index=1, frame=secToFrames(seconds))


vibeUpDownSettings = {
    "gap_length": 12.972,
    "anim_length": 3,
    "pp1_controller": bpy.data.objects['PP1_controller'],
    "pp2_controller": bpy.data.objects['PP2_controller'],
    "pp3_controller": bpy.data.objects['PP3_controller']
}

# file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/pipedream3_8_18_21_1.mid")
file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/pd1_vibe.mid")

tracks = file.getMIDITracks()
vibeTrack = file.findTrack("Vibraphone")

animator = MIDIAnimatorNode()
animator.addInstrument(midiTrack=vibeTrack, objectCollection=bpy.data.collections['Controller_Raise_Lower'], custom=VibeUpDown, customVars=vibeUpDownSettings)
animator.animate()