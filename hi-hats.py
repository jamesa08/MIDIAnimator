from __future__ import annotations
import bpy
from MIDIAnimator.utils.blender import *
from MIDIAnimator.data_structures.midi import *
from MIDIAnimator.src.animation import BlenderAnimation, Instrument
from dataclasses import dataclass
from typing import Dict
from math import radians

class Settings:
    hiHatClosed = 42
    hiHatPedal = 44
    hiHatOpen = 46

    # in degrees
    rotation = 5
     

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


class HiHatInstrument(Instrument): 
    hiHatNotes: List[MIDINote]
    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, **kwargs):
        super().__init__(midiTrack, collection, override=False)
        self.override = False  # we want to define how we want to animate it & dont use the API

        self.hiHatTopObj = None
        self.hiHatBottomObj = None

        for obj in self.collection.all_objects:
            cleanKeyframes(obj)
        deleteMarkers("debug")
        # create note table
        self.hiHatNotes = self.createHiHatNotes(midiTrack)
        self.getHiHatObjs()
        self.keyframes = []

        # thank you to TheZacher5645 in his work for figuring out the hi-hat motion
        self.hiHatAnims = {
            "none": (
            ),
            "hat-up": (
                KeyframeValue(seconds=-0.4054/2.5, value=0),
                KeyframeValue(seconds=0, value=1)
            ),
            "hat-down": (
                KeyframeValue(seconds=-0.2027/2, value=1),
                KeyframeValue(seconds=0, value=0)
            ),
            "hat-pedal": (
                KeyframeValue(seconds=-0.10135, value=1),
                KeyframeValue(seconds=0, value=0)
            ),
            "hat-pedal2": (
                KeyframeValue(seconds=-0.2027, value=0),
                KeyframeValue(seconds=-0.10135, value=1),
                KeyframeValue(seconds=0, value=0)
            ),
            "end": (
                KeyframeValue(seconds=0.4054, value=1),
                KeyframeValue(seconds=0.8108, value=0)
            )
        }
        
        # https://i.imgur.com/DhNy3KU.png
        # self.transitionMatrix[fromNote][toNote] = animation to play
        self.transitionMatrix = {
            "open": {
                "open": "none",
                "closed": "hat-down",
                "pedal": "hat-pedal",
                "end": "end"
            },
            "closed": {
                "open": "hat-up",
                "closed": "none",
                "pedal": "hat-pedal2",
                "end": "none"
            },
            "pedal": {
                "open": "hat-up",
                "closed": "none",
                "pedal": "hat-pedal2",
                "end": "none"
            },
            "start": {
                "open": "hat-up",
                "closed": "none",
                "pedal": "hat-pedal2",
                "end": "none"
            }
        }
    
        self.animType = {
            Settings.hiHatClosed: "closed",
            Settings.hiHatPedal: "pedal",
            Settings.hiHatOpen: "open"
        }

        self.hiHatRotAnims = {
            "hit": (
                KeyframeValue(seconds=-0.05, value=0),
                KeyframeValue(seconds=0, value=Settings.rotation),
                KeyframeValue(seconds=0.1, value=0)
            )
        }

        self.animate_loc()


    def evalHiHatMotion(self, curNote: MIDINote, nextNote: MIDINote, index: int) -> None:

        # CurrentNoteStart = curNote.timeOn
        # NextNoteStart = 0
        # if nextNote is not None:
        #     NextNoteStart = nextNote.timeOn
        print("\nSTARTING OPERATION.")
        print(f"{index=}")
        time = 0
        noteNumber = 0

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
            noteNumber = Settings.hiHatClosed
        
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

        for keyframe in self.hiHatAnims[animToWrite]:
            print("WRITING KEY LOC:", keyframe, "AT", secToFrames(time + keyframe.seconds))
            self.writeKey(keyframe.seconds + time, keyframe.value/10, noteNumber)
        
        # if curNote.noteNumber == Settings.hiHatClosed or curNote.noteNumber == Settings.hiHatOpen:
        #     for keyframe in self.hiHatRotAnims["hit"]:
        #         print("WRITING KEY ROT:", keyframe, "AT", secToFrames(time + keyframe.seconds))
        #         self.writeKey(keyframe.seconds + curNote.timeOn, keyframe.value, curNote.noteNumber, handleType="AUTO_CLAMPED", hit=True)
        
        print("OPERATION COMPLETE.")





    def createHiHatNotes(self, track: MIDITrack):
        hiHatNotes = []
        for note in track.notes:
            noteNumber = note.noteNumber
            if noteNumber not in {Settings.hiHatClosed, Settings.hiHatPedal, Settings.hiHatOpen}: continue
            bpy.context.scene.timeline_markers.new(name="debug", frame=int(secToFrames(note.timeOn)))

            hiHatNotes.append(note)
        
        return hiHatNotes

    def getHiHatObjs(self) -> None:
        assert len(self.collection.all_objects) == 2, "please make sure you have 2 objects in hi hat collection"
        
        for obj in self.collection.all_objects:
            if obj.name[-3:].lower() == "top":
                self.hiHatTopObj = obj
            elif obj.name[-6:].lower() == "bottom":
                self.hiHatBottomObj = obj
        
        assert self.hiHatTopObj is not None and self.hiHatBottomObj is not None, "you are missing a hi hat............"
    
    def writeKey(self, time: float, value: float, noteNumber: int, handleType: str="ALIGNED", hit=False):
        self.keyframes.append((time, value, noteNumber, handleType, hit))

    def animate_loc(self):
        # writing to the keyframeDict here
        for i, curNote in enumerate(self.hiHatNotes):
            nextNote = self.hiHatNotes[i + 1] if i+1 < len(self.hiHatNotes) else None

            # create keyframe values
            self.evalHiHatMotion(curNote, nextNote, i)

        # iterate over the keyframe dictionary to generate actual keyframes
        keyframes = self.keyframes
        for time, value, noteNumber, handleType, hit in keyframes:
            obj = self.hiHatTopObj
            if not hit:
                obj.location[2] = value # add it to itself?
                obj.keyframe_insert(data_path="location", index=2, frame=secToFrames(time))
            
            if hit:
                obj.rotation_euler[1] = radians(value)
                obj.keyframe_insert(data_path="rotation_euler", index=1, frame=secToFrames(time))
                if noteNumber == Settings.hiHatClosed:
                    self.hiHatBottomObj.rotation_euler[1] = radians(value)
                    self.hiHatBottomObj.keyframe_insert(data_path="rotation_euler", index=1, frame=secToFrames(time))
            setKeyframeHandleType(obj, handleType)

# --------------------------------------------------

file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/Drums_new.mid")
testTrack = file.findTrack("MIDI Region")

hiHats = bpy.data.collections['Hi-Hats']

animator = BlenderAnimation()
animator.addInstrument(midiTrack=testTrack, objectCollection=hiHats, custom=HiHatInstrument)
animator.addInstrument(midiTrack=testTrack, objectCollection=bpy.data.collections['cubes'])
animator.animate()