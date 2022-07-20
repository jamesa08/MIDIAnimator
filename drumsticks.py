from __future__ import annotations
from pprint import pprint
from MIDIAnimator.src.animation import BlenderAnimation
from MIDIAnimator.src.instruments import Instrument
from MIDIAnimator.data_structures.midi import *
from MIDIAnimator.utils.blender import *
from MIDIAnimator.utils import mapRange
from mathutils import Vector, Euler
from dataclasses import dataclass
from typing import Dict, List
from math import radians
import bpy

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

@dataclass
class DrumstickRanking:
    weight: float
    currentLocationOfStick: Vector
    toLocationOfStick: Vector
    distance: float
    amountOfNotesPlayed: int
    settings: DrumstickSettings
    velocity: float

@dataclass
class DrumstickSettings:
    obj: bpy.types.Object
    homePosObj: bpy.types.Object
    stickRotObj: bpy.types.Object
    listenForNotes: List[int]
    targetCollection: bpy.types.Collection
    
    animationLengthThreshold: float = 0.16
    velocityThreshold: float = 0.8

    hitMultiplierPower: float = 1.2
    
    windupTime: float = 0.233
    hitTime: float = 0.133
    recoilTime: float = 0.166
    restTime: float = 0.266

    windupValue: float = 60
    hitValue: float = 0
    recoilValue: float = 30
    restValue: float = 20
    
    def __post_init__(self):
        self.filteredNotes = []
        self.notesToPlay = []
        self.notesToPlayTable = {}
        self.noteToTargetTable = {}
        self.minVel = None
        self.maxVel = None
        self.homePos = False

        # build up noteToTarget table (one target per object for now)
        for obj in self.targetCollection.all_objects:
            note_number = int(obj.note_number)
            self.noteToTargetTable[note_number] = obj


def setKeyframeHandleType(obj: bpy.types.Object, handleType):
    with suppress(AttributeError):
        if obj is not None and obj.animation_data is not None and obj.animation_data.action is not None:
            for fCrv in FCurvesFromObject(obj):
                fCrv.keyframe_points[-1].handle_left_type = handleType
                fCrv.keyframe_points[-1].handle_right_type = handleType
"""
# @dataclass
# class DrumStickPair:
#     obj: bpy.types.Object
#     homePosObj: bpy.types.Object
#     stickRotObj: bpy.types.Object
#     # minimum number of frames between using a stick
#     _minFrameCount: int

#     # leave this out for now
#     # each key is a tuple of two notes and it maps to the minimum number of frames between playing the first note in tuple
#     # and then using the stick to play the second note in the tuple
#     _timeDict: Dict[Tuple[int, int], int]
#     # this could be computed based on the distance between the drum target locations

#     # leave this out for now
#     # maps a note number to the preferred drum stick (0 for left or 1 for right)
#     _noteDict: Dict[int, int]

#     # each tuple corresponds to the note number and the frame number the stick is used to play
#     # list of all of the times the stick is used
#     _leftStickUsed: List[Tuple[int, int]]
#     _rightStickUsed: List[Tuple[int, int]]

#     def determineSticksForNotes(self):
#         pass
#         # preprocessing
#         # for each note
#         #     get the preferred stick from _noteDict
#         #     check _left or _rightStickUsed to see if it's available to play this note by checking last entry in list
#         #     to see if number of frames necessary has passed; if so use it, otherwise see if other stick is available
#         #     if not, our minFrameCount is too high for this to be played

#     def processSticks(self):
#         pass
#         # iterate over the _leftStickUsed and make keyframes for it. can look at frame for next note and determine
#         # if should move back to home position
#         # do the same for _rightStickUsed





# class NewDrumstickInstrument(Instrument):

#     _sticks: DrumStickPair

#    # collection is a pair of drumsticks
#    # also need the dictionary indicating the preferred left/right stick for each note
#     def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, ???):
#         \"""

#         :param midiTrack:
#         :param collection:
#         :param notes: outer list length indicates number of drum stick pairs to create, and inner lists corresponds
#         to notes for each drumstick pair
#         \"""
#         self._sticks = DrumStickPair()  # pass in here more info
#         pass
#         # create the DrumstickPair objects
#         # will want an instance variable dictionary mapping each note number to the DrumstickPair that will play it

#     def animate(self, offset: int):
#         self._sticks.determineSticksForNotes()
#         self._sticks.processSticks()
"""
class DrumstickInstrumentNew(Instrument):
    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, **kwargs):
        super().__init__(midiTrack, collection, override=True)
        
        self.drumRotKeyframes = {}
        # self.noteToObjTable = {}
        # self.noteTable = {}
        self.sticksParameters = []
        self.sortNotesAlgorithim = "complex"

        for arg in kwargs:
            if arg == "stickParameters":
                self.sticksParameters = kwargs[arg]
            elif arg == "sortNotes":
                if ("simple", "complex") not in kwargs[arg]: raise RuntimeError("please choose between simple and complex sorting algorithims")
                self.sortNotesAlgorithim = kwargs[arg]
        
        self.preAnimate()
        # self.createNoteTable(midiTrack)
        self.keyframeDict = dict()

    def preAnimate(self):
        bpy.context.scene.frame_set(-10000)
        deleteMarkers("debug")

        for settings in self.sticksParameters:
            cleanKeyframes(settings.obj)
            cleanKeyframes(settings.homePosObj)
            cleanKeyframes(settings.stickRotObj)
        
    # def createNoteTable(self, track: MIDITrack):
    #     for note in track.notes:
    #         noteNumber = note.noteNumber

    #         if noteNumber in self.noteTable:
    #             self.noteTable[noteNumber].append(note)
    #         else:
    #             self.noteTable[noteNumber] = [note]
        
    # def createNoteToObjTable(self) -> None:
    #     for obj in self.collection.all_objects:
    #         if obj.note_number is None: raise RuntimeError(f"Object '{obj.name}' has no note number!")
    #         if int(obj.note_number) in self.noteToObjTable: raise RuntimeError(
    #             f"There are two objects in the scene with duplicate note numbers.")
    #         self.noteToObjTable[int(obj.note_number)] = obj

    def writekey(self, obj: bpy.types.Object, time: float, value: float, noteNumber: int, param: str=None):
        valsToWrite = (time, value, noteNumber, param)
        self.drumRotKeyframes[obj].append(valsToWrite)

    def evalDrumstickMotion(self, settings, curNote: MIDINote, nextNote: MIDINote, index: int) -> None:
        obj = settings.stickRotObj
        
        CurrentNoteStart = curNote.timeOn
        NextNoteStart = 0
        if nextNote is not None:
            NextNoteStart = nextNote.timeOn

        CurrentNoteVelocity = curNote.velocity
        NextNoteVelocity = curNote.velocity
        
        HitMultiplierPower = settings.hitMultiplierPower # use this to determine how hard the velocity element affects the range of motion
        CurrentVelocityMultiplier = (CurrentNoteVelocity/255)*HitMultiplierPower  # on a scale from 0 to 255, convert values to between 0 and HitMultiplierPower ########THIS ASSUMES 128 MEDIAN FOR ALL CHANNELS WHICH ISNT ALWAYS THE CASE
        NextVelocityMultiplier = (NextNoteVelocity/255)*HitMultiplierPower
        
        # IN SECONDS
        WindupTime = settings.windupTime
        HitTime = settings.hitTime
        RecoilTime = settings.recoilTime
        RestTime = settings.restTime
        
        WindupValue = settings.windupValue
        HitValue = settings.hitValue
        RecoilValue = settings.recoilValue
        RestValue = settings.restValue
        
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

        if index == 0:
            # first note
            #before the first note
            self.writekey(obj, (CurrentNoteStart - HitTime - WindupTime), RestValue, curNote.noteNumber) # put a key on the Rest before the next hit. we do two rests if we have enough space, an ending and a starting rest.
            self.writekey(obj, (CurrentNoteStart - HitTime), (WindupValue*NextVelocityMultiplier), curNote.noteNumber) # put a key on the windup before the next hit, using the velocitymultiplier
            self.writekey(obj, time=CurrentNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")


        if nextNote is None:
            # last note
            CurrentNoteEnd = curNote.timeOn
            #after the last note
            self.writekey(obj, (RecoilTime + CurrentNoteEnd), (RecoilValue*CurrentVelocityMultiplier), curNote.noteNumber) # put a key on the recoil after the currenthit (last hit of the drumstick)
            self.writekey(obj, (RecoilTime + RestTime + CurrentNoteEnd), RestValue, curNote.noteNumber) # put a key on the rest after the currenthit ### We've completed the currenthit
            
            print("OPERATION COMPLETE.")
            return

        if HitTimeDifference >= (TotalCycleTime): #Find the time between note hits. Find the total sequence time. If we have enough room to fit the entire thing in then,
            print("we have time for everything: ", HitTimeDifference, " >= ", TotalCycleTime)
            print("f: ", (RecoilTime + CurrentNoteEnd), "val: ", (RecoilValue*CurrentVelocityMultiplier)) # put a key on the recoil after the currenthit
            print("f: ", (RecoilTime + RestTime + CurrentNoteEnd), "val: ", RestValue) # put a key on the rest after the currenthit ### We've completed the currenthit
            print("f: ", (NextNoteStart - HitTime - WindupTime), "val: ", RestValue) # put a key on the Rest before the next hit. we do two rests if we have enough space, an ending and a starting rest.
            print("f: ", (NextNoteStart - HitTime), "val: ", (WindupValue*NextVelocityMultiplier)) # put a key on the windup before the next hit, using the velocitymultiplier
            print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!
        
            self.writekey(obj, time=(RecoilTime + CurrentNoteEnd), value=(RecoilValue*CurrentVelocityMultiplier), noteNumber=curNote.noteNumber)
            self.writekey(obj, time=(RecoilTime + RestTime + CurrentNoteEnd), value=RestValue, noteNumber=curNote.noteNumber)
            self.writekey(obj, time=(NextNoteStart - HitTime - WindupTime), value=RestValue, noteNumber=curNote.noteNumber)
            self.writekey(obj, time=(NextNoteStart - HitTime), value=(WindupValue*NextVelocityMultiplier), noteNumber=curNote.noteNumber)
            self.writekey(obj, time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")

        else:
            if HitTimeDifference > (WHRTime): # if we have time to do a hit +inverse hit + recoiltime(blue lines in img), we now have to figure out where we can put a recoil and partial rest
                print("we have time for recoil rest and windup: ", HitTimeDifference, " > ", WHRTime)
                print("f: ", (RecoilTime + CurrentNoteEnd), "val: ", (RecoilValue*CurrentVelocityMultiplier)) # put a key on the recoil after the currenthit
                RestValueCutFromSlope=(((NextNoteStart-HitTime-WindupTime)-(CurrentNoteEnd+RecoilTime)) * RestSlope) + RecoilValue # find the difference between the recoil after currenthit and Rest before nexthit and calculate where the new pre-nexthit rest would be if using the same slope.
                print("f: ", (NextNoteStart-HitTime-WindupTime), "val: ", RestValueCutFromSlope)
                print("f: ", (NextNoteStart-HitTime), "val: ", (WindupValue*NextVelocityMultiplier)) # put a key on the windup before the next hit, using the velocitymultiplier
                print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!
            

                self.writekey(obj, time=(RecoilTime + CurrentNoteEnd), value=(RecoilValue*CurrentVelocityMultiplier), noteNumber=curNote.noteNumber)
                self.writekey(obj, time=(NextNoteStart-HitTime-WindupTime), value=RestValueCutFromSlope, noteNumber=curNote.noteNumber)
                self.writekey(obj, time=(NextNoteStart-HitTime), value=(WindupValue*NextVelocityMultiplier), noteNumber=curNote.noteNumber)
                self.writekey(obj, time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")
            
            else: # if we dont have time for a full cycle or a recoil and rest, we can only do a windup before another hit
                if HitTimeDifference <= WHRTime and HitTimeDifference >= HitTime*2: #if we dont have enough for windup+hit+recoil but larger than 2Hits
                    print("We only have time for a windup that isnt weakened.", HitTimeDifference, " <= ", WHRTime, "and >= ", HitTime*2)
                    print("f: ", (NextNoteStart-HitTime), "val: ", (WindupValue*NextVelocityMultiplier)) # put a key on the windup before the next hit, using the velocitymultiplier
                    print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!
                
                    self.writekey(obj, time=(NextNoteStart-HitTime), value=(WindupValue*NextVelocityMultiplier), noteNumber=curNote.noteNumber)
                    self.writekey(obj, time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")
                else:
                    print("We only have time for a single windup that's weakened", HitTimeDifference, " < ", HitTime*2)
                    print("f: ", (HitTimeDifference/2)+CurrentNoteEnd, "val: ", (HitTimeDifference/2)*HitSlope) # if we dont even have time to do a hit +inverse hit for windup(blue lines in img), we use slope to calc how far it can go up and put it at the midpoint.
                    print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!

                    self.writekey(obj, time=(HitTimeDifference/2)+CurrentNoteEnd, value=(HitTimeDifference/2)*HitSlope, noteNumber=curNote.noteNumber)
                    self.writekey(obj, time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")

        print("OPERATION COMPLETE.")

    def evalDrumstickHitMovement(self):
        self.drumRotKeyframes = {}
        for settings in self.sticksParameters:
            obj = settings.obj  

            if settings.stickRotObj in self.drumRotKeyframes:
                raise RuntimeError(f"ERROR: There should only be one drumstick per setting class in the settings list. ")
            else:
                self.drumRotKeyframes[settings.stickRotObj] = []
            
        print(self.drumRotKeyframes)
        for settings in self.sticksParameters:
            obj = settings.stickRotObj
            for i, curNote in enumerate(settings.notesToPlay):
                # get the nextNote
                nextNote = settings.notesToPlay[i + 1] if i+1 < len(settings.notesToPlay) else curNote

                # create keyframe values
                self.evalDrumstickMotion(settings, curNote, nextNote, i)

                # add debug markers
                bpy.context.scene.timeline_markers.new("debug", frame=int(secToFrames(curNote.timeOn)))
                    

            # iterate over the keyframe dictionary to generate actual keyframes
            for obj in self.drumRotKeyframes:
                for time, value, noteNumber, param in self.drumRotKeyframes[obj]:

                    obj.rotation_euler[0] = radians(value)
                    obj.keyframe_insert(data_path="rotation_euler", index=0, frame=secToFrames(time))
                    
                    obj.location[2] = radians(value)
                    obj.keyframe_insert(data_path="location", index=2, frame=secToFrames(time)-1)

                    if param == "hit":
                        setKeyframeHandleType(obj, "VECTOR")
                    else:
                        setKeyframeHandleType(obj, "ALIGNED")

    def evalDrumstickMovement(self):
        drumstickKeyframeDict = {}
        targetsToNoteTable = dict()

        # ------------------------------------------------------------------------
        #                           KEYFRAME DICTIONARY
        # ------------------------------------------------------------------------
        
        # create keyframe dict
        for settings in self.sticksParameters:
            obj = settings.obj
            if obj in drumstickKeyframeDict:
                raise RuntimeError(f"ERROR: There should only be one drumstick per setting class in the settings list. ")
            else:
                drumstickKeyframeDict[obj] = []
        
            # ------------------------------------------------------------------------
            #                         FILTER OTHER NOTES OUT
            # ------------------------------------------------------------------------

            # check to see if any two notes are the same timeOn, if so warn
            prevNote = None
            for curNote in self.midiTrack.notes:
                if curNote.noteNumber not in settings.listenForNotes: continue

                if prevNote is not None and (prevNote.timeOn == curNote.timeOn) and (prevNote.noteNumber != curNote.noteNumber):
                    print(f"WARNING: At time {curNote.timeOn}, there are two notes assigned to 1 drumstick: {prevNote.noteNumber} and {curNote.noteNumber}.")
                prevNote = curNote
                settings.filteredNotes.append(curNote)
        
        
        # ------------------------------------------------------------------------
        #                          SORTING ALGORITHIMS
        # ------------------------------------------------------------------------

        if self.sortNotesAlgorithim == "simple":
            raise NotImplementedError("coming soon")            
            # for i, obj in enumerate(self.sticksParameters):
            #     settings = self.sticksParameters[obj]
            #     # very temp
            #     if i == 0:
            #         # left stick
            #         self.sticksParameters[obj].filteredNotes = settings.filteredNotes[::2]
            #     elif i == 1:
            #         # right stick
            #         self.sticksParameters[obj].filteredNotes = settings.filteredNotes[1::2]
        
        elif self.sortNotesAlgorithim == "complex":
            # figure out how fast each stick will be moving from note to note
            #  we need to iterate over each of the notes to get a min and max velocity (for use with mapRange())
            for settings in self.sticksParameters:
                obj = settings.obj
                velocities = []
                for i, curNote in enumerate(settings.filteredNotes):
                    nextNote = settings.filteredNotes[i + 1] if i+1 < len(settings.filteredNotes) else curNote
                    animLength = nextNote.timeOn - curNote.timeOn
                    v = velocityFromVectors(settings.noteToTargetTable[curNote.noteNumber].location, settings.noteToTargetTable[nextNote.noteNumber].location, secToFrames(animLength))
                    velocities.append(v)

                settings.maxVel, settings.minVel = max(velocities), 0


            assignedNotes = []
            for i, curNote in enumerate(self.midiTrack.notes):
                # figure out the sticks that can be played for this note
                #    if no notes can be played for this note, continue

                drumsticksForThisNote = [settings for settings in self.sticksParameters if curNote.noteNumber in settings.listenForNotes]
                
                # for settings in self.sticksParameters:
                #     if curNote.noteNumber in settings.listenForNotes: drumsticksForThisNote.append(settings)
                
                # ignore notes that aren't going to be played
                if len(drumsticksForThisNote) == 0: continue


                # determine closest stick to the targets and rank them by distance
                sticksRanked = []

                for settings in drumsticksForThisNote:
                    obj = settings.obj
                    # if there are no notes, it should be at it's home position (FIXME this may not always be the case, what if there is a note at time 0?)
                    if len(settings.notesToPlay) == 0:
                        lastPlayedNoteForThisStick = None
                        getCurrentLocationOfStick = settings.homePosObj.location
                    else:
                        lastPlayedNoteForThisStick = settings.notesToPlay[-1]
                        getCurrentLocationOfStick = settings.noteToTargetTable[lastPlayedNoteForThisStick.noteNumber].location
                    
                    getToLocationOfStick = settings.noteToTargetTable[curNote.noteNumber].location
                    
                    distance = distanceFromVectors(getToLocationOfStick, getCurrentLocationOfStick)

                    amountOfNotesPlayed = 1
                    if curNote.noteNumber in settings.notesToPlayTable:
                        # this value needs to be inverted somehow
                        amountOfNotesPlayed = len(settings.notesToPlayTable[curNote.noteNumber]) + 1
                    
                    if lastPlayedNoteForThisStick is None:
                        lastNotePlayedTimeOn = 0
                    else:
                        lastNotePlayedTimeOn = lastPlayedNoteForThisStick.timeOn
                    
                    animLength = curNote.timeOn - lastNotePlayedTimeOn
                    velocity = velocityFromVectors(getToLocationOfStick, getCurrentLocationOfStick, secToFrames(animLength))
                    velocityRel = mapRange(velocity, settings.minVel, settings.maxVel, 0, 5)
                    
                    # this determines what stick to use for this note
                    # best case drumstick
                    # short distance, long amount of notes played, small velocity
                    summedWeight = (1 + (distance + velocity)) / amountOfNotesPlayed
                    # summedWeight = distance * amountOfNotesPlayed * velocity

                    sticksRanked.append(
                        DrumstickRanking(
                            summedWeight,
                            getCurrentLocationOfStick, 
                            getToLocationOfStick, 
                            distance, 
                            amountOfNotesPlayed, 
                            settings, 
                            velocityRel
                        )
                    )
                
                sticksRanked.sort(key=lambda ranking: ranking.weight)
                
                # debug
                print(secToFrames(curNote.timeOn))
                pprint(sticksRanked, depth=1)
                print()

                assignedStick = False
                for ranking in sticksRanked:
                    distance = ranking.distance
                    settings = ranking.settings
                    getCurrentLocationOfStick = ranking.currentLocationOfStick
                    getToLocationOfStick = ranking.toLocationOfStick
                    amountOfNotesPlayed = ranking.amountOfNotesPlayed
                    
                    obj = settings.obj

                    if len(settings.notesToPlay) == 0:
                        lastPlayedNote = self.midiTrack.notes[0]
                    else:
                        lastPlayedNote = settings.notesToPlay[-1]


                    animLength = curNote.timeOn - lastPlayedNote.timeOn
                    velocity = velocityFromVectors(getCurrentLocationOfStick, getToLocationOfStick, secToFrames(animLength))

                    # if the clostest stick's last played note is within some threshold

                    # debug stuff
                    s = ""
                    velocityRel = mapRange(velocity, settings.minVel, settings.maxVel, 0, 5)
                    if velocityRel <= settings.velocityThreshold: s = "velocity within range"
                    # print(f"sec={curNote.timeOn} frames={secToFrames(curNote.timeOn)} {animLength=} mapRange={mapRange(velocity, settings.minVel, settings.maxVel, 0, 5)} {s}")


                    if (animLength >= settings.animationLengthThreshold and mapRange(velocity, settings.minVel, settings.maxVel, 0, 5) <= settings.velocityThreshold):
                        # assign it to stick
                        if curNote not in assignedNotes:
                            settings.notesToPlay.append(curNote)
                            
                            if curNote.noteNumber in settings.notesToPlayTable:
                                settings.notesToPlayTable[curNote.noteNumber].append(curNote)
                            else:
                                settings.notesToPlayTable[curNote.noteNumber] = [curNote]
                            
                            assignedNotes.append(curNote)
                            assignedStick = True
                            
                            break

                if not assignedStick:
                    print(f"WARNING: No other sticks can play this note '{curNote}'. It can move with a animLength of {animLength} and a velocity of {velocityRel}")
                    # print(f"WARNING: Assinging note anyway to closest drumstick... ")
                    # settings = self.sticksParameters[sticksRankedByDistance[0][1]]
                    # settings.notesToPlay.append(curNote)

        # ------------------------------------------------------------------------
        #                    INSERT KEYFRAMES INTO DICTIONARY
        # ------------------------------------------------------------------------

        # make sticks go to home position on first note
        for settings in self.sticksParameters:
            obj = settings.obj
            settings.homePos = True
            # insert keyframe at 0 (XXX what will happen if notes are at 0?)
            drumstickKeyframeDict[obj].append(
                    BlenderKeyframe(
                        location=settings.homePosObj.location, 
                        rotation=settings.homePosObj.rotation_euler,
                        frame=0
                    )
            )

        # insert keyframes here
        for settings in self.sticksParameters:
            obj = settings.obj
            alreadyWarn = False
            for i, curNote in enumerate(settings.notesToPlay):
                prevNote = settings.notesToPlay[i - 1] if i - 1 != 0 else curNote
                nextNote = settings.notesToPlay[i + 1] if i+1 < len(settings.notesToPlay) else curNote

                animLength = nextNote.timeOn - curNote.timeOn
                
                # return to home pos
                if animLength >= 10 or i == len(settings.notesToPlay) - 1:
                    # hold
                    # FIXME uncommment 
                    drumstickKeyframeDict[obj].append(
                        BlenderKeyframe(
                            location=settings.noteToTargetTable[curNote.noteNumber].location, 
                            rotation=settings.noteToTargetTable[curNote.noteNumber].rotation_euler,
                            frame=secToFrames(curNote.timeOn + 0.5)
                        )
                    )

                    # move back to home pos
                    drumstickKeyframeDict[obj].append(
                        BlenderKeyframe(
                            location=settings.homePosObj.location, 
                            rotation=settings.homePosObj.rotation_euler,
                            frame=secToFrames(curNote.timeOn + 1.5)
                        )
                    )
                    settings.homePos = True
                    pass
                
                # print(secToFrames(curNote.timeOn), mapRange(velocities[i], minV, maxV, 0, 5))
                
                drumstickKeyframeDict[obj].append(
                        BlenderKeyframe(
                            location=settings.noteToTargetTable[curNote.noteNumber].location, 
                            rotation=settings.noteToTargetTable[curNote.noteNumber].rotation_euler,
                            frame=secToFrames(curNote.timeOn)
                        )
                    )


                # write a second key to make the motion feel natural (shortening the length of it)
                fromVector = settings.noteToTargetTable[curNote.noteNumber].location
                fromRot = settings.noteToTargetTable[curNote.noteNumber].rotation_euler
                toVector = settings.noteToTargetTable[nextNote.noteNumber].location
                # FIXME uncomment
                if settings.homePos:
                    fromVector = settings.homePosObj.location
                    fromRot = settings.homePosObj.rotation_euler
                    settings.homePos = False
                
                # first note fix
                if i == 0:
                    toVector = settings.noteToTargetTable[curNote.noteNumber].location
                    settings.homePos = False
                # FIXME uncomment all down
                animLength = timeFromVectors(fromVector, toVector, mapRange(3, settings.minVel, settings.maxVel, 0, 5))
                
                
                if animLength > framesToSec(animLength):    # why does it have to be in framesToSec()???
                    if i == 0:
                        frame = secToFrames(curNote.timeOn) - animLength
                    else:
                        frame = secToFrames(nextNote.timeOn) - animLength
                 
                    # drumstickKeyframeDict[obj].append(
                    #         BlenderKeyframe(
                    #             location=fromVector, 
                    #             rotation=fromRot,
                    #             frame=frame
                    #         )
                    # )
                else:
                    if not alreadyWarn:
                        print(f"WARNING: Drumstick '{obj.name}' will be a little fast (beyond 3 m/s). Consider creating an additional drumstick.")
                        alreadyWarn = True


        # ------------------------------------------------------------------------
        #                        WRITE KEYFRAMES TO BLENDER
        # ------------------------------------------------------------------------

        # insert actual keyframes
        for obj in drumstickKeyframeDict:
            for keyframe in drumstickKeyframeDict[obj]:
                obj.location = keyframe.location
                obj.rotation_euler = keyframe.rotation

                obj.keyframe_insert(data_path="location", frame=keyframe.frame)
                obj.keyframe_insert(data_path="rotation_euler", frame=keyframe.frame)
            
    def animate(self):
        self.evalDrumstickMovement()
        self.evalDrumstickHitMovement()

"""
class DrumstickInstrument(Instrument): 
    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, **kwargs):
        super().__init__(midiTrack, collection)
        self.override = True  # we want to define how we want to animate it & dont use the API
        self.drumRotKeyframes = {}
        self.noteToObjTable = {}

        # self.sticks2ListenNotes = [38, 40, 42, 44, 46, 55]
        
        self.sticksParameters = {
            bpy.data.objects['drumstick1L']: DrumstickSettings(
                obj=bpy.data.objects['drumstick1L'],
                homePosObj=bpy.data.objects['home_position_1'],
                stickRotObj=bpy.data.objects['StickLRot'],
                listenForNotes=[40, 41, 43, 45, 48, 49, 50, 57],
                targetCollection=bpy.data.collections['CubeTargets']
            ),
            bpy.data.objects['drumstick1R']: DrumstickSettings(
                obj=bpy.data.objects['drumstick1R'],
                homePosObj=bpy.data.objects['home_position_2'],
                stickRotObj=bpy.data.objects['StickRRot'],
                listenForNotes=[40, 41, 43, 45, 48, 49, 50, 57],
                targetCollection=bpy.data.collections['CubeTargets']
            )
        }

        self.preAnimate()
        # create note table
        self.noteTable = self.createNoteTable(midiTrack)
        # self.createNoteToObjTable()
        self.keyframeDict = dict()

    def preAnimate(self):
        bpy.context.scene.frame_set(0)
        for obj in self.collection.all_objects:
            settings = self.sticksParameters[obj]
            cleanKeyframes(settings.obj)
            cleanKeyframes(settings.homePosObj)
            cleanKeyframes(settings.stickRotObj)
        
        deleteMarkers("debug")

    def evalDrumstickMotion(self, obj, curNote: MIDINote, nextNote: MIDINote, index: int) -> None:
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
            self.writekey(obj, (CurrentNoteStart - HitTime - WindupTime), RestValue, curNote.noteNumber) # put a key on the Rest before the next hit. we do two rests if we have enough space, an ending and a starting rest.
            self.writekey(obj, (CurrentNoteStart - HitTime), (WindupValue*NextVelocityMultiplier), curNote.noteNumber) # put a key on the windup before the next hit, using the velocitymultiplier
            self.writekey(obj, time=CurrentNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")


        if nextNote is None:
            # last note
            CurrentNoteEnd = curNote.timeOn
            #after the last note
            self.writekey(obj, (RecoilTime + CurrentNoteEnd), (RecoilValue*CurrentVelocityMultiplier), curNote.noteNumber) # put a key on the recoil after the currenthit (last hit of the drumstick)
            self.writekey(obj, (RecoilTime + RestTime + CurrentNoteEnd), RestValue, curNote.noteNumber) # put a key on the rest after the currenthit ### We've completed the currenthit
            
            print("OPERATION COMPLETE.")
            return

        if HitTimeDifference >= (TotalCycleTime): #Find the time between note hits. Find the total sequence time. If we have enough room to fit the entire thing in then,
            print("we have time for everything: ", HitTimeDifference, " >= ", TotalCycleTime)
            print("f: ", (RecoilTime + CurrentNoteEnd), "val: ", (RecoilValue*CurrentVelocityMultiplier)) # put a key on the recoil after the currenthit
            print("f: ", (RecoilTime + RestTime + CurrentNoteEnd), "val: ", RestValue) # put a key on the rest after the currenthit ### We've completed the currenthit
            print("f: ", (NextNoteStart - HitTime - WindupTime), "val: ", RestValue) # put a key on the Rest before the next hit. we do two rests if we have enough space, an ending and a starting rest.
            print("f: ", (NextNoteStart - HitTime), "val: ", (WindupValue*NextVelocityMultiplier)) # put a key on the windup before the next hit, using the velocitymultiplier
            print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!
        
            self.writekey(obj, time=(RecoilTime + CurrentNoteEnd), value=(RecoilValue*CurrentVelocityMultiplier), noteNumber=curNote.noteNumber)
            self.writekey(obj, time=(RecoilTime + RestTime + CurrentNoteEnd), value=RestValue, noteNumber=curNote.noteNumber)
            self.writekey(obj, time=(NextNoteStart - HitTime - WindupTime), value=RestValue, noteNumber=curNote.noteNumber)
            self.writekey(obj, time=(NextNoteStart - HitTime), value=(WindupValue*NextVelocityMultiplier), noteNumber=curNote.noteNumber)
            self.writekey(obj, time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")

        else:
            if HitTimeDifference > (WHRTime): # if we have time to do a hit +inverse hit + recoiltime(blue lines in img), we now have to figure out where we can put a recoil and partial rest
                print("we have time for recoil rest and windup: ", HitTimeDifference, " > ", WHRTime)
                print("f: ", (RecoilTime + CurrentNoteEnd), "val: ", (RecoilValue*CurrentVelocityMultiplier)) # put a key on the recoil after the currenthit
                RestValueCutFromSlope=(((NextNoteStart-HitTime-WindupTime)-(CurrentNoteEnd+RecoilTime)) * RestSlope) + RecoilValue # find the difference between the recoil after currenthit and Rest before nexthit and calculate where the new pre-nexthit rest would be if using the same slope.
                print("f: ", (NextNoteStart-HitTime-WindupTime), "val: ", RestValueCutFromSlope)
                print("f: ", (NextNoteStart-HitTime), "val: ", (WindupValue*NextVelocityMultiplier)) # put a key on the windup before the next hit, using the velocitymultiplier
                print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!
            

                self.writekey(obj, time=(RecoilTime + CurrentNoteEnd), value=(RecoilValue*CurrentVelocityMultiplier), noteNumber=curNote.noteNumber)
                self.writekey(obj, time=(NextNoteStart-HitTime-WindupTime), value=RestValueCutFromSlope, noteNumber=curNote.noteNumber)
                self.writekey(obj, time=(NextNoteStart-HitTime), value=(WindupValue*NextVelocityMultiplier), noteNumber=curNote.noteNumber)
                self.writekey(obj, time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")
            
            else: # if we dont have time for a full cycle or a recoil and rest, we can only do a windup before another hit
                if HitTimeDifference <= WHRTime and HitTimeDifference >= HitTime*2: #if we dont have enough for windup+hit+recoil but larger than 2Hits
                    print("We only have time for a windup that isnt weakened.", HitTimeDifference, " <= ", WHRTime, "and >= ", HitTime*2)
                    print("f: ", (NextNoteStart-HitTime), "val: ", (WindupValue*NextVelocityMultiplier)) # put a key on the windup before the next hit, using the velocitymultiplier
                    print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!
                
                    self.writekey(obj, time=(NextNoteStart-HitTime), value=(WindupValue*NextVelocityMultiplier), noteNumber=curNote.noteNumber)
                    self.writekey(obj, time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")
                else:
                    print("We only have time for a single windup that's weakened", HitTimeDifference, " < ", HitTime*2)
                    print("f: ", (HitTimeDifference/2)+CurrentNoteEnd, "val: ", (HitTimeDifference/2)*HitSlope) # if we dont even have time to do a hit +inverse hit for windup(blue lines in img), we use slope to calc how far it can go up and put it at the midpoint.
                    print("f: ", NextNoteStart, "val: ", HitValue, "HIT!") # Key the next hit. #SPECIFYTheKeyTangents/Handles!!!!

                    self.writekey(obj, time=(HitTimeDifference/2)+CurrentNoteEnd, value=(HitTimeDifference/2)*HitSlope, noteNumber=curNote.noteNumber)
                    self.writekey(obj, time=NextNoteStart, value=HitValue, noteNumber=curNote.noteNumber, param="hit")

        print("OPERATION COMPLETE.")

    def createNoteTable(self, track: MIDITrack):
        outDict = dict()
        for note in track.notes:
            noteNumber = note.noteNumber

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
    
    def writekey(self, obj: bpy.types.Object, time: float, value: float, noteNumber: int, param: str=None):
        valsToWrite = (time, value, noteNumber, param)
        self.drumRotKeyframes[obj].append(valsToWrite)

    def animate(self):
        drumstickKeyframeDict = {}
        targetsToNoteTable = dict()

        # create keyframe dict
        for obj in self.sticksParameters:
            settings = self.sticksParameters[obj]
            if obj in drumstickKeyframeDict:
                raise RuntimeError(f"ERROR: There should only be one drumstick per setting class in the settings list. ")
            else:
                drumstickKeyframeDict[obj] = []
        
            # filter the notes 
            # check to see if any two notes are the same timeOn, if so warn
            prevNote = None
            for curNote in self.midiTrack.notes:
                if curNote.noteNumber not in settings.listenForNotes: continue

                if prevNote is not None and (prevNote.timeOn == curNote.timeOn) and (prevNote.noteNumber != curNote.noteNumber):
                    print(f"WARNING: At time {curNote.timeOn}, there are two notes assigned to 1 drumstick: {prevNote.noteNumber} and {curNote.noteNumber}.")
                prevNote = curNote
                settings.filteredNotes.append(curNote)
        

        # figure out how fast each stick will be moving from note to note
        # first we need to iterate over each of the notes to get a min and max velocity (for use with mapRange())
        for obj in self.sticksParameters:
            settings = self.sticksParameters[obj]
            velocities = []
            for i, curNote in enumerate(settings.filteredNotes):
                nextNote = settings.filteredNotes[i + 1] if i+1 < len(settings.filteredNotes) else curNote
                animLength = nextNote.timeOn - curNote.timeOn
                v = velocityFromVectors(settings.noteToTargetTable[curNote.noteNumber].location, settings.noteToTargetTable[nextNote.noteNumber].location, secToFrames(animLength))
                velocities.append(v)

            settings.maxVel, settings.minVel = max(velocities), 0


        for i, curNote in enumerate(self.midiTrack.notes):
            # prevNote = self.midiTrack.notes[i - 1] if i - 1 != 0 else curNote
            # figure out the sticks that can be played for this note
            #    if no notes can be played for this note, continue

            drumsticksForThisNote = []
            for obj in self.sticksParameters:
                settings = self.sticksParameters[obj]
                if curNote.noteNumber in settings.listenForNotes:
                    drumsticksForThisNote.append(obj)
            
            # ignore notes that aren't going to be played
            if len(drumsticksForThisNote) == 0: continue
            
            # determine closest stick to the targets and rank them by distance
            sticksRankedByDistance = []
            for obj in self.sticksParameters:
                settings = self.sticksParameters[obj]
                if len(settings.notesToPlay) == 0:
                    getCurrentLocationOfStick = settings.homePosObj.location
                else:
                    lastPlayedNoteForThisStick = settings.notesToPlay[-1]
                    getCurrentLocationOfStick = settings.noteToTargetTable[lastPlayedNoteForThisStick.noteNumber].location
                
                getToLocationOfStick = settings.noteToTargetTable[curNote.noteNumber].location
                
                distance = abs(distanceFromVectors(getToLocationOfStick, getCurrentLocationOfStick))
                sticksRankedByDistance.append((distance, obj, getCurrentLocationOfStick, getToLocationOfStick))
            
            sticksRankedByDistance.sort(key=lambda tup: tup[0])


            assignedStick = False
            for distance, obj, getCurrentLocationOfStick, getToLocationOfStick in sticksRankedByDistance:
                settings = self.sticksParameters[obj]
                if len(settings.notesToPlay) == 0:
                    lastPlayedNote = self.midiTrack.notes[0]
                else:
                    lastPlayedNote = settings.notesToPlay[-1]


                animLength = curNote.timeOn - lastPlayedNote.timeOn
                velocity = velocityFromVectors(getCurrentLocationOfStick, getToLocationOfStick, secToFrames(animLength))
                # if the clostest stick's last played note is within some threshold

                if animLength == 0:
                    # note is at the same time, we need to assign it to the other stick 
                    pass
                
                # if animLength >= 0.1 and not assignedStick:
                s = ""
                velocityRel = mapRange(velocity, settings.minVel, settings.maxVel, 0, 5)
                if velocityRel <= 0.8:
                    s = "velocity within range"
                print(f"sec={curNote.timeOn} frames={secToFrames(curNote.timeOn)} {animLength=} mapRange={mapRange(velocity, settings.minVel, settings.maxVel, 0, 5)} {s}")

                if animLength >= 0.16 and mapRange(velocity, settings.minVel, settings.maxVel, 0, 5) <= 0.8 and obj in drumsticksForThisNote:
                    alreadyAssignedToStick = False
                    for _, checkObj, _, _ in sticksRankedByDistance:
                        if checkObj == obj: continue
                        checkSettings = self.sticksParameters[checkObj]
                        if curNote in checkSettings.notesToPlay: alreadyAssignedToStick = True; break
                    
                    if not alreadyAssignedToStick:
                        settings.notesToPlay.append(curNote)
                        assignedStick = True
                        break
                
                if not assignedStick:
                    print(f"WARNING: No other sticks can play this note '{curNote}'. It can move with a animLength of {animLength} and a velocity of {velocityRel}")
                    # print(f"WARNING: Assinging note anyway to closest drumstick... ")
                    # settings = self.sticksParameters[sticksRankedByDistance[0][1]]
                    # settings.notesToPlay.append(curNote)
            
        
            # stickIndex = 0
            # assignedStick = False
            # while stickIndex < len(sticksRankedByDistance):
            #     obj = sticksRankedByDistance[i][1]
            #     settings = self.sticksParameters[obj]
            #     lastPlayedNote = settings.filteredNotes[-1]
            #     if len(settings.filteredNotes) == 0:
            #         lastPlayedNote = float('inf') 
            #     delta = curNote.timeOn - lastPlayedNote.timeOn
            #     # if the clostest stick's last played note is within some threshold
            #     if delta >= 0.35:
            #         settings.filteredNotes.append(curNote)
            #         assignedStick = True
            #         break
            
            # if not assignedStick:
            #     print(f"WARNING: No other sticks can play this note {curNote}. Consider adding another drumstick.")
            #     print(f"WARNING: Assinging note anyway to closest drumstick... ")
            #     settings = self.sticksParameters[sticksRankedByDistance[0]]
            #     settings.filteredNotes.append(curNote)
            
        
        # need a way to split the notes up
        # lets split in half for now
        # later, we will have a more complex way of splitting the notes up
        # for i, obj in enumerate(self.sticksParameters):
        #     settings = self.sticksParameters[obj]
        #     # very temp
        #     # need a better way of telling L from R
        #     if i == 0:
        #         # left stick
        #         self.sticksParameters[obj].filteredNotes = settings.filteredNotes[::2]
        #     elif i == 1:
        #         # right stick
        #         self.sticksParameters[obj].filteredNotes = settings.filteredNotes[1::2]


        # make sticks go to home position on first note
        for obj in self.sticksParameters:
            settings = self.sticksParameters[obj]
            settings.homePos = True
            # insert keyframe at 0 (XXX what will happen if notes are at 0?)
            drumstickKeyframeDict[obj].append(
                    BlenderKeyframe(
                        location=settings.homePosObj.location, 
                        rotation=settings.homePosObj.rotation_euler,
                        frame=0
                    )
            )

        # insert keyframes here
        for obj in self.sticksParameters:
            alreadyWarn = False
            settings = self.sticksParameters[obj]
            for i, curNote in enumerate(settings.notesToPlay):
                prevNote = settings.notesToPlay[i - 1] if i - 1 != 0 else curNote
                nextNote = settings.notesToPlay[i + 1] if i+1 < len(settings.notesToPlay) else curNote

                animLength = nextNote.timeOn - curNote.timeOn
                
                # return to home pos
                if animLength >= 10 or i == len(settings.notesToPlay) - 1:
                    # hold
                    # FIXME uncommment 
                    drumstickKeyframeDict[obj].append(
                        BlenderKeyframe(
                            location=settings.noteToTargetTable[curNote.noteNumber].location, 
                            rotation=settings.noteToTargetTable[curNote.noteNumber].rotation_euler,
                            frame=secToFrames(curNote.timeOn + 0.5)
                        )
                    )

                    # move back to home pos
                    drumstickKeyframeDict[obj].append(
                        BlenderKeyframe(
                            location=settings.homePosObj.location, 
                            rotation=settings.homePosObj.rotation_euler,
                            frame=secToFrames(curNote.timeOn + 1.5)
                        )
                    )
                    settings.homePos = True
                    pass
                
                # print(secToFrames(curNote.timeOn), mapRange(velocities[i], minV, maxV, 0, 5))
                
                drumstickKeyframeDict[obj].append(
                        BlenderKeyframe(
                            location=settings.noteToTargetTable[curNote.noteNumber].location, 
                            rotation=settings.noteToTargetTable[curNote.noteNumber].rotation_euler,
                            frame=secToFrames(curNote.timeOn)
                        )
                    )


                # write a second key to make the motion feel natural (shortening the length of it)
                fromVector = settings.noteToTargetTable[curNote.noteNumber].location
                fromRot = settings.noteToTargetTable[curNote.noteNumber].rotation_euler
                toVector = settings.noteToTargetTable[nextNote.noteNumber].location
                # FIXME uncomment
                if settings.homePos:
                    fromVector = settings.homePosObj.location
                    fromRot = settings.homePosObj.rotation_euler
                    settings.homePos = False
                
                # first note fix
                if i == 0:
                    toVector = settings.noteToTargetTable[curNote.noteNumber].location
                    settings.homePos = False
                # FIXME uncomment all down
                animLength = timeFromVectors(fromVector, toVector, mapRange(3, settings.minVel, settings.maxVel, 0, 5))
                
                
                if animLength > framesToSec(animLength):    # why does it have to be in framesToSec()???
                    if i == 0:
                        frame = secToFrames(curNote.timeOn) - animLength
                    else:
                        frame = secToFrames(nextNote.timeOn) - animLength
                 
                    # drumstickKeyframeDict[obj].append(
                    #         BlenderKeyframe(
                    #             location=fromVector, 
                    #             rotation=fromRot,
                    #             frame=frame
                    #         )
                    # )
                else:
                    if not alreadyWarn:
                        print(f"WARNING: Drumstick '{obj.name}' will be a little fast (beyond 3 m/s). Consider creating an additional drumstick.")
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
        self.drumRotKeyframes = {}
        for obj in self.sticksParameters:
            settings = self.sticksParameters[obj]    

            if settings.stickRotObj in self.drumRotKeyframes:
                raise RuntimeError(f"ERROR: There should only be one drumstick per setting class in the settings list. ")
            else:
                self.drumRotKeyframes[settings.stickRotObj] = []
            
        
        for obj in self.sticksParameters:
            settings = self.sticksParameters[obj]    
            # for noteNumber in self.noteTable:
            for i, curNote in enumerate(settings.notesToPlay):
                # get the nextNote
                # nextNote = self.noteTable[noteNumber][i + 1] if i+1 < len(self.noteTable[noteNumber]) else None
                nextNote = settings.notesToPlay[i + 1] if i+1 < len(settings.notesToPlay) else curNote

                # create keyframe values
                self.evalDrumstickMotion(settings.stickRotObj, curNote, nextNote, i)

                # add debug markers
                bpy.context.scene.timeline_markers.new("debug", frame=int(secToFrames(curNote.timeOn)))
                    

            # iterate over the keyframe dictionary to generate actual keyframes
            for obj in self.drumRotKeyframes:
                for time, value, noteNumber, param in self.drumRotKeyframes[obj]:

                    obj.rotation_euler[0] = radians(value)
                    obj.keyframe_insert(data_path="rotation_euler", index=0, frame=secToFrames(time))
                    
                    obj.location[2] = radians(value)
                    obj.keyframe_insert(data_path="location", index=2, frame=secToFrames(time)-1)

                    if param == "hit":
                        setKeyframeHandleType(obj, "VECTOR")
                    else:
                        setKeyframeHandleType(obj, "ALIGNED")

"""
# --------------------------------------------------

#file = MIDIFile("/Users/james/Downloads/test_midi.mid")
#testTrack = file.findTrack("test_track")

file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/StickFiguresNewMIDI.mid")
testTrack = file._tracks[0]
# print(testTrack)

drumsticks = bpy.data.collections['DrumstickGimbals']

# quick notes to objs
scene = bpy.context.scene
scene.quick_instrument_type = "custom"
scene.note_number_list = str([1, 2, 3, 4])
scene.quick_obj_col = bpy.data.collections['DrumstickGimbals']
scene.quick_obj_curve = bpy.data.objects['ANIM_curve']
scene.quick_use_sorted = True
bpy.ops.scene.quick_add_props()

scene.quick_instrument_type = "custom"
scene.note_number_list = str([38, 40, 41, 42, 43, 45, 46, 48, 49, 50, 57])
scene.quick_obj_col = bpy.data.collections['CubeTargets']
scene.quick_obj_curve = bpy.data.objects['ANIM_curve']
scene.quick_use_sorted = False
bpy.ops.scene.quick_add_props()

settings = {
    "stickParameters": [
        DrumstickSettings(
            obj=bpy.data.objects['drumstick1L'],
            homePosObj=bpy.data.objects['home_position_1'],
            stickRotObj=bpy.data.objects['StickLRot'],
            listenForNotes=[40, 41, 43, 45, 48, 50, 57],
            targetCollection=bpy.data.collections['CubeTargets'],
            # animationLengthThreshold=0.07,
            # velocityThreshold=1.43
        ),
        DrumstickSettings(
            obj=bpy.data.objects['drumstick1R'],
            homePosObj=bpy.data.objects['home_position_2'],
            stickRotObj=bpy.data.objects['StickRRot'],
            listenForNotes=[40, 41, 43, 45, 48, 50, 57],
            targetCollection=bpy.data.collections['CubeTargets'],
            # animationLengthThreshold=0.07,
            # velocityThreshold=1.43
        ),
        DrumstickSettings(
            obj=bpy.data.objects['drumstick2L'],
            homePosObj=bpy.data.objects['home_position_1L'],
            stickRotObj=bpy.data.objects['StickLRot2'],
            listenForNotes=[38, 42, 46, 49],
            targetCollection=bpy.data.collections['CubeTargets'],
            animationLengthThreshold=0.08,
            velocityThreshold=3.75
        ),
        DrumstickSettings(
            obj=bpy.data.objects['drumstick2R'],
            homePosObj=bpy.data.objects['home_position_2R'],
            stickRotObj=bpy.data.objects['StickRRot2'],
            listenForNotes=[38, 42, 46, 49],
            targetCollection=bpy.data.collections['CubeTargets'],
            animationLengthThreshold=0.08,
            velocityThreshold=3.75
        )
    ]
}


animator = BlenderAnimation()
animator.addInstrument(midiTrack=testTrack, objectCollection=drumsticks, custom=DrumstickInstrumentNew, customVars=settings)  # Drumsticks
# animator.addInstrument(midiTrack=testTrack, objectCollection=bpy.data.collections['Cubes'])  #Cubes
animator.animate()