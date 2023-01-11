import bpy
from copy import deepcopy
from MIDIAnimator.data_structures import FrameRange
from MIDIAnimator.src.animation import MIDIAnimatorNode
from MIDIAnimator.data_structures.midi import MIDIFile, MIDITrack, MIDINote
from MIDIAnimator.src.instruments import Instrument
from MIDIAnimator.utils.blender import *
from MIDIAnimator.utils import *
from dataclasses import dataclass
from pprint import pprint

@dataclass
class Keyframe:
    frame: float
    value: float

    def __hash__(self) -> int:
        return hash((self.frame, self.value))

# def findIntersection(x1, y1, x2, y2, x3, y3, x4, y4):
#     try:
#         pX = (((x1*y2)-(y1*x2)) * (x3-x4) - (x1-x2) * ((x3*y4)-(y3*x4))) / (((x1-x2) * (y3-y4)) - ((y1-y2) * (x3 - x4)))
#         pY = (((x1*y2)-(y1*x2)) * (y3-y4) - (y1-y2) * ((x3*y4)-(y3*x4))) / (((x1-x2) * (y3-y4)) - ((y1-y2) * (x3 - x4)))
#     except ZeroDivisionError:
#         return 0, 0
    
#     return pX, pY

# def findInterval(frame, alreadyInsertedKeyframes):
#     if len(alreadyInsertedKeyframes) < 2: return
#     if frame < alreadyInsertedKeyframes[0].frame: return
#     if frame > alreadyInsertedKeyframes[-1].frame: return

#     out = []
#     for i, insertedKey in enumerate(alreadyInsertedKeyframes):
        
#         # if insertedKey.frame <= nextKeys[0].frame: continue
#         if insertedKey.frame <= frame: continue

#         # for nextKey in nextKeys:
#         if frame <= insertedKey.frame:
#             out.append((alreadyInsertedKeyframes[i-1], i-1))
#             out.append((alreadyInsertedKeyframes[i], i))
#             break
            
#     return out

def findOverlap(keyList1, keyList2):
    if len(keyList1) == 0 or len(keyList2) == 0:
        return []
    
    if keyList1[0].frame > keyList2[0].frame:
        #this means a note is somehow going back in time? is this even possible?
        # notes should always be sequential, and not in reverse time
        raise ValueError("first keyframe in keyList1 is bigger than first keyframe in keyList2! Please open a issue on GitHub along with the MIDI file.")
    
    overlappingKeyList = []
    overlapping = False
    for key1 in reversed(keyList1):
        # print ("testing case key1={keyl.frame} and key2={keyList2[0].frame}")
        if key1.frame > keyList2[0].frame:
            overlapping = True
            overlappingKeyList.append(key1)
        else:
            # not overlapping
            if overlapping:
                overlappingKeyList.append(key1)
            break

    return list(reversed(overlappingKeyList))

def getValue(key1: Keyframe, key2: Keyframe, frame: float) -> float:
    x1, y1 = key1.frame, key1.value
    x2, y2 = key2.frame, key2.value    
    try:
        m = (y2 - y1) / (x2 - x1)
    except ZeroDivisionError:
        # i dont know if this will work every time
        m = 0
    
    c = y1 - m * x1
    return (m * frame) + c

def interval(keyList, frame):
    if len(keyList) == 0: 
        return (None, None)
    if keyList[0].frame > frame:
        # out of range to the left of the list
        return (keyList[0], keyList[0])
    elif keyList[-1].frame < frame:
        # out of range to the right of the list
        return (keyList[-1], keyList[-1])
    
    for i in range(len(keyList)):
        if keyList[i].frame <= frame <= keyList[i+1].frame:
            return (keyList[i], keyList[i+1])

class NoteOffTesting(Instrument):
    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, **kwargs):
        super().__init__(midiTrack, collection, override=True)
        self.override = True
        self.preAnimate()
        
        noteOnCurves = self.makeObjToFCurveDict(type="note_on")
        noteOffCurves = self.makeObjToFCurveDict(type="note_off")
        self.createNoteToBlenderObject(noteOnCurves, noteOffCurves)
        # self.frameRanges = self.calculateFrameRanges()

    def preAnimate(self):
        bpy.context.scene.frame_set(-10000)
        deleteMarkers("NOTE")
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)

    def calculateFrameRanges(self) -> FrameRange:
        out = []
        for note in self.midiTrack.notes:
            if note.noteNumber in self.noteToBlenderObject:
                wprs = self.noteToBlenderObject[note.noteNumber]
            else:
                # warn that there is no object tied to note number in MIDI
                continue
            
            bpy.context.scene.timeline_markers.new(f'NOTE ON {note.timeOn}', frame=int(secToFrames(note.timeOn)))
            bpy.context.scene.timeline_markers.new(f'NOTE OFF {note.timeOff}', frame=int(secToFrames(note.timeOff)))

            for wpr in wprs:
                obj = wpr.obj
                objMidi = obj.midi
                
                rangeOn, rangeOff = 0.0, 0.0
                
                if objMidi.anim_curve_type == "keyframed":

                    if objMidi.note_on_curve:
                        # get the relaitive time the curve is on
                        start, end = wpr.rangeOn()
                        rangeOn = end - start
                        
                        rangeOn += objMidi.note_on_anchor_pt
                        rangeOn += secToFrames(note.timeOn)
                    
                    if objMidi.note_off_curve:
                        # get the relaitive time the curve is off
                        start, end = wpr.rangeOff()
                        rangeOff = end - start
                        
                        rangeOff += objMidi.note_off_anchor_pt
                        rangeOff += secToFrames(note.timeOff)
                
                elif objMidi.anim_curve_type == "damp_osc":
                    pass
                
                elif objMidi.anim_curve_type == "adsr":
                    pass
                
                out.append(FrameRange(rangeOn, rangeOff, wpr))
        
        return out

    
    def animate(self):
        alreadyInserted = []
        
        print("x, y")

        for note in self.midiTrack.notes:
            if note.noteNumber in self.noteToBlenderObject:
                wprs = self.noteToBlenderObject[note.noteNumber]
            else:
                # warn that there is no object tied to note number in MIDI
                continue
            
            bpy.context.scene.timeline_markers.new(f'NOTE ON {note.timeOn}', frame=int(secToFrames(note.timeOn)))
            bpy.context.scene.timeline_markers.new(f'NOTE OFF {note.timeOff}', frame=int(secToFrames(note.timeOff)))

            for wpr in wprs:

                # for shapeName in wpr.noteOffCurves.shapeKeysDict:
                #     shapeKey = wpr.noteOnCurves.shapeKeysDict[shapeName][1]

                obj = wpr.obj
                objMidi = obj.midi
                
                rangeOn, rangeOff = 0.0, 0.0
                
                if objMidi.anim_curve_type == "keyframed":

                    if objMidi.note_on_curve:
                        # get the relative time the curve is on
                        start, end = wpr.rangeOn()
                        rangeOn = start
                        
                        rangeOn += objMidi.note_on_anchor_pt
                        rangeOn += secToFrames(note.timeOn)
                    

                    if objMidi.note_off_curve:
                        # get the relative time the curve is off
                        start, end = wpr.rangeOff()
                        rangeOff = start
                        
                        rangeOff += objMidi.note_off_anchor_pt
                        rangeOff += secToFrames(note.timeOff)

                
                elif objMidi.anim_curve_type == "damp_osc":
                    pass
                
                elif objMidi.anim_curve_type == "adsr":
                    pass




        insertedKeys = []

        for note in self.midiTrack.notes:
            if note.noteNumber in self.noteToBlenderObject:
                wprs = self.noteToBlenderObject[note.noteNumber]
            else:
                # TODO warn
                continue
            
            for wpr in wprs:
                nextKeys = []
                # note on
                if wpr.obj.midi.note_on_curve:
                    for shapeName in wpr.noteOnCurves.shapeKeysDict:
                        fCrv = wpr.noteOnCurves.shapeKeysDict[shapeName][0]  # reference shape key
                        shapeKey = wpr.noteOnCurves.shapeKeysDict[shapeName][1]  # wpr shape key
                        for keyframe in fCrv.keyframe_points:
                            frame = keyframe.co[0] + secToFrames(note.timeOn) + wpr.obj.midi.note_on_anchor_pt
                            value = keyframe.co[1]
                            nextKeys.append(Keyframe(frame, value))
                
                if wpr.obj.midi.note_off_curve:
                    # note off
                    for shapeName in wpr.noteOffCurves.shapeKeysDict:
                        fCrv = wpr.noteOffCurves.shapeKeysDict[shapeName][0]  # reference shape key
                        shapeKey = wpr.noteOffCurves.shapeKeysDict[shapeName][1]  # wpr shape key
                        for keyframe in fCrv.keyframe_points:
                            frame = keyframe.co[0] + secToFrames(note.timeOff) + wpr.obj.midi.note_off_anchor_pt
                            value = keyframe.co[1]
                            nextKeys.append(Keyframe(frame,value))

                # x = 0
                # keyList1 = [Keyframe(frame=59, value=0.0), Keyframe(frame=74, value=0.5), Keyframe(frame=77, value=0.5), Keyframe(frame=92, value=0.0)]
                # keyList2 = [Keyframe(frame=89-x, value=0.0), Keyframe(frame=104-x, value=0.5), Keyframe(frame=107-x, value=0.5), Keyframe(frame=122-x, value=0.0)]
                
                keyList1Overlapping = findOverlap(insertedKeys, nextKeys)

                key1InterpolatedValues = []
                key2InterpolatedValues = []

                # interpolate the keyframes for each graph
                for key2 in nextKeys:
                    key1interval1, key1interval2 = interval(keyList1Overlapping, key2.frame)
                    if key1interval1 is None and key1interval2 is None: 
                        continue
                    key2InterpolatedValues.append(Keyframe(key2.frame, getValue(key1interval1, key1interval2, key2.frame)))

                for key1 in keyList1Overlapping:
                    key2interval1, key2interval2 = interval(nextKeys, key1.frame)
                    if key2interval1 is None and key2interval2 is None: 
                        continue
                    key1InterpolatedValues.append(Keyframe(key1.frame, getValue(key2interval1, key2interval2, key1.frame)))

                # now add the keyframe values together (the most important part)
                for key1, key1Interp in zip(keyList1Overlapping, key1InterpolatedValues):
                    key1.value += key1Interp.value

                for key2, key2Interp in zip(nextKeys, key2InterpolatedValues):
                    key2.value += key2Interp.value

                # extend the lists (need a better method to ensure the keyframes before get cut off and then start )
                keyList1Overlapping.extend(nextKeys)
                keyList1Overlapping.sort(key=lambda keyframe: keyframe.frame)

                insertedKeys.extend(keyList1Overlapping)
                
                # insertedKeys = sorted(insertedKeys, key=lambda keyframe: keyframe.frame)
                # if len(insertedKeys) != 0 and insertedKeys[-1].frame > nextKeys[0].frame:
                #     # if the last insertedKeys keyframe is greater than the first added keyframe, then they are overlapping
                #     # need to find out where the overlapping point is
                #     print(insertedKeys[-1].frame - nextKeys[0].frame)
                # else:
                #     # no keyframes in inserted, first note likely
                #     insertedKeys.extend(nextKeys)



        for keyframe in sorted(insertedKeys, key=lambda keyframe: keyframe.frame):
            print(f"{keyframe.frame},{keyframe.value}")
            shapeKey.value = keyframe.value
            shapeKey.keyframe_insert(data_path="value", frame=keyframe.frame)
        

file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/test_midi_2notes_fixed.mid")
test = file.findTrack("Test")

animator = MIDIAnimatorNode()
animator.addInstrument(instrumentType="custom", midiTrack=test, objectCollection=bpy.data.collections['NewSystem'], custom=NoteOffTesting)

# Animate the MIDI file
animator.animate()


# for key in shapeKeyFCurvesFromObject(bpy.data.objects['Cubes'])[0].keyframe_points:
#     print(f"{key.co[0]},{key.co[1]}")
