from MIDIAnimator.src.animation import BlenderAnimation
from MIDIAnimator.src.instruments import Instrument
from MIDIAnimator.data_structures.midi import *
from MIDIAnimator.utils import mapRange
from MIDIAnimator.utils.blender import *
from dataclasses import dataclass
from math import radians
from typing import Dict
import bpy
from mathutils import Vector, Euler

class Settings:
    onSpeed = 60   # ms
    onValue = -1
    
    offSpeed = 400  # ms
    offValue = 0
    
    # define values
    _valueDiff = onValue - offValue
    onslope = _valueDiff / onSpeed
    offslope = _valueDiff / offSpeed
    velocity = 0
     
@dataclass
class BlenderKeyframe:
    location: Vector
    rotation: Euler
    frame: float


def setKeyframeHandleType(obj: bpy.types.Object, handleType):
    with suppress(AttributeError):
        if obj is not None and obj.animation_data is not None and obj.animation_data.action is not None:
            for fCrv in FCurvesFromObject(obj):
                fCrv.keyframe_points[-1].handle_left_type = handleType
                fCrv.keyframe_points[-1].handle_right_type = handleType


class DrumstickInstrument(Instrument): 
    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection):
        super().__init__(midiTrack, collection)
        self.override = True  # we want to define how we want to animate it & dont use the API
        self.noteToObjTable = {}

        self.sticks1ListenNotes = [41, 43, 45, 47, 48, 49, 57]
        self.sticks2ListenNotes = [38, 40, 42, 44, 46, 55]

        self.preAnimate()
        # create note table
        self.noteTable = self.createNoteTable(midiTrack)
        # self.createNoteToObjTable()
        self.keyframeDict = dict()    

    def preAnimate(self):
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)
        deleteMarkers("debug")

    def evalDrumstickMotion(self, curNote: MIDINote, nextNote: MIDINote, index: int) -> None:

        CurrentNoteStart = curNote.timeOn
        NextNoteStart = 0
        if nextNote is not None:
            NextNoteStart = nextNote.timeOn

        CurrentNoteVelocity = 128
        NextNoteVelocity = 128
        
        HitMultiplierPower = 1.2 #############Pull from DrumstickBPYObject. use this to determine how hard the velocity element affects the range of motion
        CurrentVelocityMultiplier = (CurrentNoteVelocity/255)*HitMultiplierPower #on a scale from 0 to 255, convert values to between 0 and HitMultiplierPower ########THIS ASSUMES 128 MEDIAN FOR ALL CHANNELS WHICH ISNT ALWAYS THE CASE
        NextVelocityMultiplier = (NextNoteVelocity/255)*HitMultiplierPower
        
        # IN SECONDS
        WindupTime = 0.233
        HitTime = 0.133
        RecoilTime = 0.166
        RestTime = 0.266
        
        WindupValue = 60
        HitValue = 0
        RecoilValue = 30
        RestValue = 20
        
        CurrentNoteEnd = CurrentNoteStart #If we dont want the drumstick to stick to the pad during a note, which is not usually wanted, then we just say that in this function consider the end of the currentnote to be the start - when the hit occurred.
        
        ### I'm putting these variables here  but we'll be importing them from settings
        
        HitSlope = (abs(HitValue-WindupValue) / HitTime)
        RestSlope = (RestValue-RecoilValue) / RestTime
        
        CurrentVelocityMultiplier = 1
        NextVelocityMultiplier = 1 #worry abt velocity later
        
        HitTimeDifference = NextNoteStart - CurrentNoteEnd
        
        TotalCycleTime = WindupTime+HitTime+RecoilTime+RestTime
        WHRTime = WindupTime+HitTime+RecoilTime
        print("STARTING OPERATION.")

        if index == 0:  #XXX i changed this from i to index.. will it break?
            # first note
            #before the first note
            self.writekey((CurrentNoteStart - HitTime - WindupTime), RestValue, curNote.noteNumber) # put a key on the Rest before the next hit. we do two rests if we have enough space, an ending and a starting rest.
            self.writekey((CurrentNoteStart - HitTime), (WindupValue*NextVelocityMultiplier), curNote.noteNumber) # put a key on the windup before the next hit, using the velocitymultiplier
            self.writekey(time=CurrentNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")


        if nextNote is None:
            # last note
            CurrentNoteEnd = curNote.timeOn
            #after the last note
            self.writekey((RecoilTime + CurrentNoteEnd), (RecoilValue*CurrentVelocityMultiplier), curNote.noteNumber) # put a key on the recoil after the currenthit (last hit of the drumstick)
            self.writekey((RecoilTime + RestTime + CurrentNoteEnd), RestValue, curNote.noteNumber) # put a key on the rest after the currenthit ### We've completed the currenthit
            
            print("OPERATION COMPLETE.")
            return

        if HitTimeDifference >= (TotalCycleTime): #Find the time between note hits. Find the total sequence time. If we have enough room to fit the entire thing in then,
            print("we have time for everything: ", HitTimeDifference, " >= ", TotalCycleTime)
            print("f: ", (RecoilTime + CurrentNoteEnd), "val: ", (RecoilValue*CurrentVelocityMultiplier)) # put a key on the recoil after the currenthit
            print("f: ", (RecoilTime + RestTime + CurrentNoteEnd), "val: ", RestValue) # put a key on the rest after the currenthit ### We've completed the currenthit
            print("f: ", (NextNoteStart - HitTime - WindupTime), "val: ", RestValue) # put a key on the Rest before the next hit. we do two rests if we have enough space, an ending and a starting rest.
            print("f: ", (NextNoteStart - HitTime), "val: ", (WindupValue*NextVelocityMultiplier)) # put a key on the windup before the next hit, using the velocitymultiplier
            print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!
        
            self.writekey(time=(RecoilTime + CurrentNoteEnd), value=(RecoilValue*CurrentVelocityMultiplier), noteNumber=curNote.noteNumber)
            self.writekey(time=(RecoilTime + RestTime + CurrentNoteEnd), value=RestValue, noteNumber=curNote.noteNumber)
            self.writekey(time=(NextNoteStart - HitTime - WindupTime), value=RestValue, noteNumber=curNote.noteNumber)
            self.writekey(time=(NextNoteStart - HitTime), value=(WindupValue*NextVelocityMultiplier), noteNumber=curNote.noteNumber)
            self.writekey(time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")

        else:
            if HitTimeDifference > (WHRTime): # if we have time to do a hit +inverse hit + recoiltime(blue lines in img), we now have to figure out where we can put a recoil and partial rest
                print("we have time for recoil rest and windup: ", HitTimeDifference, " > ", WHRTime)
                print("f: ", (RecoilTime + CurrentNoteEnd), "val: ", (RecoilValue*CurrentVelocityMultiplier)) # put a key on the recoil after the currenthit
                RestValueCutFromSlope=(((NextNoteStart-HitTime-WindupTime)-(CurrentNoteEnd+RecoilTime)) * RestSlope) + RecoilValue # find the difference between the recoil after currenthit and Rest before nexthit and calculate where the new pre-nexthit rest would be if using the same slope.
                print("f: ", (NextNoteStart-HitTime-WindupTime), "val: ", RestValueCutFromSlope)
                print("f: ", (NextNoteStart-HitTime), "val: ", (WindupValue*NextVelocityMultiplier)) # put a key on the windup before the next hit, using the velocitymultiplier
                print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!
            

                self.writekey(time=(RecoilTime + CurrentNoteEnd), value=(RecoilValue*CurrentVelocityMultiplier), noteNumber=curNote.noteNumber)
                self.writekey(time=(NextNoteStart-HitTime-WindupTime), value=RestValueCutFromSlope, noteNumber=curNote.noteNumber)
                self.writekey(time=(NextNoteStart-HitTime), value=(WindupValue*NextVelocityMultiplier), noteNumber=curNote.noteNumber)
                self.writekey(time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")
            
            else: # if we dont have time for a full cycle or a recoil and rest, we can only do a windup before another hit
                if HitTimeDifference <= WHRTime and HitTimeDifference >= HitTime*2: #if we dont have enough for windup+hit+recoil but larger than 2Hits
                    print("We only have time for a windup that isnt weakened.", HitTimeDifference, " <= ", WHRTime, "and >= ", HitTime*2)
                    print("f: ", (NextNoteStart-HitTime), "val: ", (WindupValue*NextVelocityMultiplier)) # put a key on the windup before the next hit, using the velocitymultiplier
                    print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!
                
                    self.writekey(time=(NextNoteStart-HitTime), value=(WindupValue*NextVelocityMultiplier), noteNumber=curNote.noteNumber)
                    self.writekey(time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")
                else:
                    print("We only have time for a single windup that's weakened", HitTimeDifference, " < ", HitTime*2)
                    print("f: ", (HitTimeDifference/2)+CurrentNoteEnd, "val: ", (HitTimeDifference/2)*HitSlope) # if we dont even have time to do a hit +inverse hit for windup(blue lines in img), we use slope to calc how far it can go up and put it at the midpoint.
                    print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!

                    self.writekey(time=(HitTimeDifference/2)+CurrentNoteEnd, value=(HitTimeDifference/2)*HitSlope, noteNumber=curNote.noteNumber)
                    self.writekey(time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")

        print("OPERATION COMPLETE.")

    def createNoteTable(self, track: MIDITrack):
        outDict = dict()
        for note in track.notes:
            noteNumber = note.noteNumber
            if noteNumber not in self.sticks1ListenNotes: continue

            if noteNumber in outDict:
                outDict[noteNumber].append(note)
            else:
                outDict[noteNumber] = [note]
        
        return outDict

    def createNoteToObjTable(self) -> None:
        for obj in self.collection.all_objects:
            if obj.note_number is None: raise RuntimeError(f"Object '{obj.name}' has no note number!")
            if int(obj.note_number) in self.noteToObjTable: raise RuntimeError(
                f"There are two objects in the scene with duplicate note numbers.")
            self.noteToObjTable[int(obj.note_number)] = obj
    
    def writekey(self, time: float, value: float, noteNumber: int, param: str=None):
        keyframeDict = self.keyframeDict
        valsToWrite = (time, value, noteNumber, param)

        if noteNumber in keyframeDict:
            keyframeDict[noteNumber].append(valsToWrite)
        else:
            keyframeDict[noteNumber] = [valsToWrite]

    def animate(self):
        drumstickKeyframeDict = {
            bpy.data.objects['drumstick1L']: []
        }
        filteredNotes = []
        targetsToNoteTable = dict()

        # filter the notes 
        for note in self.midiTrack.notes:
            if note.noteNumber not in self.sticks1ListenNotes: continue
            filteredNotes.append(note)
    

        for obj in bpy.data.collections['CubeTargets'].all_objects:
            note_number = int(obj.note_number)
            targetsToNoteTable[note_number] = obj

        velocities = []
        for i, curNote in enumerate(filteredNotes):
            nextNote = filteredNotes[i + 1] if i+1 < len(filteredNotes) else curNote
            animLength = nextNote.timeOn - curNote.timeOn
            v = velocityFromVectors(targetsToNoteTable[curNote.noteNumber].location, targetsToNoteTable[nextNote.noteNumber].location, secToFrames(animLength))
            velocities.append(v)

        maxV, minV = max(velocities), 0

        # make sticks go to home position on first note
        obj = bpy.data.objects['drumstick1L']
        # insert keyframe at 0 (XXX what will happen if notes are at 0?)
        drumstickKeyframeDict[obj].append(
                BlenderKeyframe(
                    location=bpy.data.objects['home_position_1'].location, 
                    rotation=bpy.data.objects['home_position_1'].rotation_euler,
                    frame=0
                )
        )
        
        # calculate time from 0 to the first note (filteredNotes[0])

        homePos = True

        # insert keyframes here
        alreadyWarn = False
        for i, curNote in enumerate(filteredNotes):
            prevNote = filteredNotes[i - 1] if i - 1 != 0 else curNote
            nextNote = filteredNotes[i + 1] if i+1 < len(filteredNotes) else curNote

            animLength = nextNote.timeOn - curNote.timeOn
            
            # return to home pos
            if animLength >= 5 or i == len(filteredNotes) - 1:
                obj = bpy.data.objects['drumstick1L']
                
                # hold
                drumstickKeyframeDict[obj].append(
                    BlenderKeyframe(
                        location=targetsToNoteTable[curNote.noteNumber].location, 
                        rotation=targetsToNoteTable[curNote.noteNumber].rotation_euler,
                        frame=secToFrames(curNote.timeOn + 0.5)
                    )
                )

                # move back to home pos
                drumstickKeyframeDict[obj].append(
                    BlenderKeyframe(
                        location=bpy.data.objects['home_position_1'].location, 
                        rotation=bpy.data.objects['home_position_1'].rotation_euler,
                        frame=secToFrames(curNote.timeOn + 1.5)
                    )
                )
                homePos = True
                # continue
            
            # print(secToFrames(curNote.timeOn), mapRange(velocities[i], minV, maxV, 0, 5))
            
            obj = bpy.data.objects['drumstick1L']
            drumstickKeyframeDict[obj].append(
                    BlenderKeyframe(
                        location=targetsToNoteTable[curNote.noteNumber].location, 
                        rotation=targetsToNoteTable[curNote.noteNumber].rotation_euler,
                        frame=secToFrames(curNote.timeOn)
                    )
                )

            # write a second key to make the motion feel natural (shortening the length of it)
            fromVector = targetsToNoteTable[curNote.noteNumber].location
            fromRot = targetsToNoteTable[curNote.noteNumber].rotation_euler
            toVector = targetsToNoteTable[nextNote.noteNumber].location
            
            if homePos:
                fromVector = bpy.data.objects['home_position_1'].location
                fromRot = bpy.data.objects['home_position_1'].rotation_euler
                homePos = False
            
            # first note fix
            if i == 0:
                toVector = targetsToNoteTable[curNote.noteNumber].location
                homePos = False

            delta = timeFromVectors(fromVector, toVector, mapRange(3, minV, maxV, 0, 5))
            
            
            if animLength > framesToSec(delta):    # why does it have to be in framesToSec()???
                if i == 0:
                    frame = secToFrames(curNote.timeOn) - delta
                else:
                    frame = secToFrames(nextNote.timeOn) - delta
                
                drumstickKeyframeDict[obj].append(
                        BlenderKeyframe(
                            location=fromVector, 
                            rotation=fromRot,
                            frame=frame
                        )
                )
            else:
                if not alreadyWarn:
                    print(f"WARNING: your drumsticks will be a little fast (beyond 3 m/s).... might want another drumstick :)")
                    alreadyWarn = True





        # insert actual keyframes
        for obj in drumstickKeyframeDict:
            for keyframe in drumstickKeyframeDict[obj]:
                obj.location = keyframe.location
                obj.rotation_euler = keyframe.rotation

                obj.keyframe_insert(data_path="location", frame=keyframe.frame)
                obj.keyframe_insert(data_path="rotation_euler", frame=keyframe.frame)
            
        self.animateOld()

    def animateOld(self):
        # writing to the keyframeDict here
        filteredNotes = []

        # filter the notes 
        for note in self.midiTrack.notes:
            if note.noteNumber not in self.sticks1ListenNotes: continue
            filteredNotes.append(note)

        
        # for noteNumber in self.noteTable:
        for i, curNote in enumerate(filteredNotes):
            # get the nextNote
            # nextNote = self.noteTable[noteNumber][i + 1] if i+1 < len(self.noteTable[noteNumber]) else None
            nextNote = filteredNotes[i + 1] if i+1 < len(filteredNotes) else curNote

            # create keyframe values
            self.evalDrumstickMotion(curNote, nextNote, i)

            # add debug markers
            bpy.context.scene.timeline_markers.new("debug", frame=int(secToFrames(curNote.timeOn)))
                

        # iterate over the keyframe dictionary to generate actual keyframes
        keyframeDict = self.keyframeDict
        for key in keyframeDict:
            for time, value, noteNumber, param in keyframeDict[key]:
                # if noteNumber not in self.noteToObjTable: continue
                # obj = self.noteToObjTable[noteNumber]
                obj = bpy.data.objects['StickLRot']
                
                obj.rotation_euler[0] = radians(value)
                obj.keyframe_insert(data_path="rotation_euler", index=0, frame=secToFrames(time))
                
                obj.location[2] = radians(value)
                obj.keyframe_insert(data_path="location", index=2, frame=secToFrames(time)-1)

                if param == "hit":
                    setKeyframeHandleType(obj, "VECTOR")
                else:
                    setKeyframeHandleType(obj, "ALIGNED")


# --------------------------------------------------

#file = MIDIFile("/Users/james/Downloads/test_midi.mid")
#testTrack = file.findTrack("test_track")

file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/StickFiguresMIDI.mid")
testTrack = file._tracks[0]
# print(testTrack)

drumsticks = bpy.data.collections['DrumstickGimbals']

# quick notes to objs
scene = bpy.context.scene
scene.quick_instrument_type = "custom"
scene.note_number_list = str([1, 2])
scene.quick_obj_col = bpy.data.collections['DrumstickGimbals']
scene.quick_obj_curve = bpy.data.objects['ANIM_curve']
scene.quick_use_sorted = True
bpy.ops.scene.quick_add_props()

scene.quick_instrument_type = "custom"
scene.note_number_list = str([38, 41, 42, 43, 45, 47, 48, 49, 57])
scene.quick_obj_col = bpy.data.collections['CubeTargets']
scene.quick_obj_curve = bpy.data.objects['ANIM_curve']
scene.quick_use_sorted = False
bpy.ops.scene.quick_add_props()

animator = BlenderAnimation()
animator.addInstrument(midiTrack=testTrack, objectCollection=drumsticks, custom=DrumstickInstrument)  # Drumsticks
# animator.addInstrument(midiTrack=testTrack, objectCollection=bpy.data.collections['Cubes'])  #Cubes
animator.animate()