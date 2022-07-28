from __future__ import annotations
import bpy
from MIDIAnimator.utils.blender import *
from MIDIAnimator.data_structures.midi import *
from MIDIAnimator.src.animation import MIDIAnimatorNode, Instrument
from dataclasses import dataclass
from typing import Dict
from math import radians

"""
Drum notes correspondence:

hi -> lo
Tom1  = D2  50
Tom2  = C2  48
Tom3  = B1  47
Tom4  = A1  45
Tom5  = G1  43
Tom6  = F1  41

Kick  = C1  36
Snare = D1  38
CymbL = C#2 49
CymbR = A2  57

WblkH = B2  59
WblkL = C3  60
Splsh = G2  55
Cwbll = G#2 56

ClsHH = F#1 42
PdlHH = G#1 44
OpnHH = A#1 46

"""

class Settings:
    hiHatClosed = 42
    hiHatOpen = 46
    
    woodblockH = 59
    woodblockL = 60

    splash = 55

    cowbell = 56

def setKeyframeHandleType(obj: bpy.types.Object, handleType):
    with suppress(AttributeError):
        if obj is not None and obj.animation_data is not None and obj.animation_data.action is not None:
            for fCrv in FCurvesFromObject(obj):
                fCrv.keyframe_points[-1].handle_left_type = handleType
                fCrv.keyframe_points[-1].handle_right_type = handleType

@dataclass
class KeyframeValue:
    seconds: float
    value: float


class FourWayPercussion(Instrument): 
    fourWayNotes: List[MIDINote]

    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, **kwargs):
        super().__init__(midiTrack, collection, override=True)
        self.override = True  # we want to define how we want to animate it & dont use the API

        self.animType = {
            Settings.hiHatClosed: "hi-hat",
            Settings.hiHatOpen: "hi-hat", 
            Settings.woodblockH: "woodblock", 
            Settings.woodblockL: "woodblock", 
            Settings.splash: "splash", 
            Settings.cowbell: "cowbell"
        }

        self.rotObj = None

        self.preAnimate()
        # create note table
        self.fourWayNotes = self.filterFourWayNotes(midiTrack)
        self.getRotObj()
        self.keyframes = []

        # thank you to TheZacher5645 in his work for figuring out the four way perc motion
        self.fourWayAnims = {
            "none": (),
            "pos-90": (
                KeyframeValue(seconds=-1, value=0),
                KeyframeValue(seconds=0, value=90)
            ),
            "pos-180": (
                KeyframeValue(seconds=-1, value=0),
                KeyframeValue(seconds=0, value=180)
            ),
            "neg-90": (
                KeyframeValue(seconds=-1, value=0),
                KeyframeValue(seconds=0, value=-90)
            ),
            "neg-180": (
                KeyframeValue(seconds=-1, value=0),
                KeyframeValue(seconds=0, value=-180)
            ),
        }
        
        # https://i.imgur.com/33pUNEq.png
        # self.transitionMatrix[fromNote][toNote] = animation to play

        self.transitionMatrix = {
            "splash": {
                "splash": "none",
                "hi-hat": "neg-90",
                "cowbell": "pos-180",
                "woodblock": "pos-90",
                "end": "none"
            },
            "hi-hat": {
                "splash": "pos-90",
                "hi-hat": "none",
                "cowbell": "neg-90",
                "woodblock": "neg-180",
                "end": "pos-90"
            },
            "cowbell": {
                "splash": "neg-180",
                "hi-hat": "pos-90",
                "cowbell": "none",
                "woodblock": "neg-90",
                "end": "neg-180"
            },
            "woodblock": {
                "splash": "neg-90",
                "hi-hat": "pos-180",
                "cowbell": "pos-90",
                "woodblock": "none",
                "end": "neg-90"
            },
            "start": {
                "splash": "none",
                "hi-hat": "neg-90",
                "cowbell": "pos-180",
                "woodblock": "pos-90",
                "end": "none"
            }
        }

        

    def preAnimate(self):
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)
        deleteMarkers("debug")

    def evalRotMotion(self, curNote: MIDINote, nextNote: MIDINote, index: int) -> None:

        # CurrentNoteStart = curNote.timeOn
        # NextNoteStart = 0
        # if nextNote is not None:
        #     NextNoteStart = nextNote.timeOn
        print("\nSTARTING OPERATION.")
        print(f"{index=}")
        time = 0
        noteNumber = 0
        interpolationType = "BEZIER"

        curNoteType = self.animType[curNote.noteNumber]

        if nextNote is not None:
            # normal note (not first or last)
            nextNoteType = self.animType[nextNote.noteNumber]
            time = nextNote.timeOn
            noteNumber = nextNote.noteNumber
        

        if nextNote is None:
            # last note
            print("LAST NOTE")
            nextNoteType = "end"
            time = curNote.timeOn
            noteNumber = curNote.noteNumber
        
        if index == 0:
            # first note
            print("FIRST NOTE")
            curNoteType = "start"
            time = curNote.timeOn
            noteNumber = curNote.noteNumber
        

        animToWrite = self.transitionMatrix[curNoteType][nextNoteType]
        print("ANIM TO WRITE: ", animToWrite)

        if nextNote is not None:
            animDuration = nextNote.timeOn - curNote.timeOn
        else:
            animDuration = 1
        print("ANIM DURATION", animDuration)
        
        keyframeDuration = 1
        if animToWrite != "none":
            keyframeDuration = self.fourWayAnims[animToWrite][-1].seconds - self.fourWayAnims[animToWrite][0].seconds
        
        speedMult = 1
        if index != 0 and animDuration < keyframeDuration:
            speedMult = animDuration / keyframeDuration
        
        print("SPEED MULT", speedMult)
        
        if index != 0 and animDuration < 0.5:
            interpolationType = "BACK"

        for keyframe in self.fourWayAnims[animToWrite]:
            print("WRITING KEY ROT:", keyframe, "AT", secToFrames((keyframe.seconds * speedMult) + time))
            self.writeKey((keyframe.seconds * speedMult) + time, keyframe.value, noteNumber, handleType="AUTO_CLAMPED", interpolation=interpolationType)
        
        # if curNote.noteNumber == Settings.hiHatClosed or curNote.noteNumber == Settings.hiHatOpen:
        #     for keyframe in self.hiHatRotAnims["hit"]:
        #         print("WRITING KEY ROT:", keyframe, "AT", secToFrames(time + keyframe.seconds))
        #         self.writeKey(keyframe.seconds + curNote.timeOn, keyframe.value, curNote.noteNumber, handleType="AUTO_CLAMPED")
        
        print("OPERATION COMPLETE.")





    def filterFourWayNotes(self, track: MIDITrack):
        fourWayNotes = []
        for note in track.notes:
            noteNumber = note.noteNumber
            if noteNumber not in self.animType: continue
            # bpy.context.scene.timeline_markers.new(name="debug", frame=int(secToFrames(note.timeOn)))

            fourWayNotes.append(note)
        
        return fourWayNotes

    def getRotObj(self) -> None:
        assert len(self.collection.all_objects) == 1, "please make sure you have 1 objects in the collection"
        
        for obj in self.collection.all_objects:
            self.rotObj = obj
    
    def writeKey(self, time: float, value: float, noteNumber: int, handleType: str="AUTO_CLAMPED", interpolation="BEZIER"):
        self.keyframes.append((time, value, noteNumber, handleType, interpolation))

    def animate(self):
        # writing to the keyframeDict here
        for i, curNote in enumerate(self.fourWayNotes):
            nextNote = self.fourWayNotes[i + 1] if i+1 < len(self.fourWayNotes) else None

            # create keyframe values
            self.evalRotMotion(curNote, nextNote, i)

        # iterate over the keyframe dictionary to generate actual keyframes
        keyframes = self.keyframes
        valueSummed = 0
        for time, value, noteNumber, handleType, interpolation in keyframes:
            obj = self.rotObj
            valueSummed += value
            obj.rotation_euler.z = radians(valueSummed)
            obj.keyframe_insert(data_path="rotation_euler", index=2, frame=secToFrames(time))
            
            # if hit:
            #     obj.rotation_euler[1] = radians(value)
            #     obj.keyframe_insert(data_path="rotation_euler", index=1, frame=secToFrames(time))
            #     if noteNumber == Settings.hiHatClosed:
            #         self.hiHatBottomObj.rotation_euler[1] = radians(value)
            #         self.hiHatBottomObj.keyframe_insert(data_path="rotation_euler", index=1, frame=secToFrames(time))
            setKeyframeHandleType(obj, handleType)
            setKeyframeInterpolation(obj, interpolation)

# --------------------------------------------------

file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/Drums_new.mid")
testTrack = file.findTrack("MIDI Region")

rotationObject = bpy.data.collections['rotation_object']

animator = MIDIAnimatorNode()
animator.addInstrument(midiTrack=testTrack, objectCollection=rotationObject, custom=FourWayPercussion)
# animator.addInstrument(midiTrack=testTrack, objectCollection=bpy.data.collections['cubes'])
animator.animate()