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

def findOverlap(keyList1, keyList2):
    if len(keyList1) == 0 or len(keyList2) == 0:
        return []
    
    if keyList1[0].frame > keyList2[0].frame:
        # this means a note is somehow going back in time? is this even possible?
        # notes should always be sequential, and not in reverse time
        raise ValueError("first keyframe in keyList1 is bigger than first keyframe in keyList2! Please open a issue on GitHub along with the MIDI file.")
    
    overlappingKeyList = []
    overlapping = False
    for key1 in reversed(keyList1):
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
        
        noteOnCurves = self.makeObjToFCurveDict(noteType="note_on")
        noteOffCurves = self.makeObjToFCurveDict(noteType="note_off")
        self.createNoteToBlenderObject(noteOnCurves, noteOffCurves)
        # self.frameRanges = self.calculateFrameRanges()

    def preAnimate(self):
        bpy.context.scene.frame_set(-10000)
        deleteMarkers("NOTE")
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)
    """
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
    """
    
    def animate(self):
        for note in self.midiTrack.notes:
            if note.noteNumber in self.noteToBlenderObject:
                wprs = self.noteToBlenderObject[note.noteNumber]
            else:
                # warn that there is no object tied to note number in MIDI
                continue
            
            # bpy.context.scene.timeline_markers.new(f'NOTE ON {note.timeOn}', frame=int(secToFrames(note.timeOn)))
            # bpy.context.scene.timeline_markers.new(f'NOTE OFF {note.timeOff}', frame=int(secToFrames(note.timeOff)))

            # for wpr in wprs:
            #     obj = wpr.obj
            #     objMidi = obj.midi
                
            #     rangeOn, rangeOff = 0.0, 0.0
                
            #     if objMidi.anim_curve_type == "keyframed":

            #         if objMidi.note_on_curve:
            #             # get the relative time the curve is on
            #             start, end = wpr.rangeOn()
            #             rangeOn = start
                        
            #             rangeOn += objMidi.note_on_anchor_pt
            #             rangeOn += secToFrames(note.timeOn)
                    

            #         if objMidi.note_off_curve:
            #             # get the relative time the curve is off
            #             start, end = wpr.rangeOff()
            #             rangeOff = start
                        
            #             rangeOff += objMidi.note_off_anchor_pt
            #             rangeOff += secToFrames(note.timeOff)

                
            #     elif objMidi.anim_curve_type == "damp_osc":
            #         pass
                
            #     elif objMidi.anim_curve_type == "adsr":
            #         pass

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

                keysOverlapping = findOverlap(insertedKeys, nextKeys)

                insertedKeysInterValues = []
                nextKeysInterValues = []

                # interpolate the keyframes for each set of keyframes
                for key in nextKeys:
                    inv1, inv2 = interval(keysOverlapping, key.frame)
                    if inv1 is None and inv2 is None: continue
                    nextKeysInterValues.append(Keyframe(key.frame, getValue(inv1, inv2, key.frame)))

                for key in keysOverlapping:
                    inv1, inv2 = interval(nextKeys, key.frame)
                    if inv1 is None and inv2 is None: continue
                    insertedKeysInterValues.append(Keyframe(key.frame, getValue(inv1, inv2, key.frame)))

                # now add the keyframe values together (the most important part)
                for key, interp in zip(keysOverlapping, insertedKeysInterValues):
                    key.value += interp.value

                for key, interp in zip(nextKeys, nextKeysInterValues):
                    key.value += interp.value

                # extend the lists (need a better method to ensure the keyframes before get cut off and then start )
                keysOverlapping.extend(nextKeys)
                keysOverlapping.sort(key=lambda keyframe: keyframe.frame)

                insertedKeys.extend(keysOverlapping)

        for keyframe in sorted(insertedKeys, key=lambda keyframe: keyframe.frame):
            # print(f"{keyframe.frame},{keyframe.value}")
            shapeKey.value = keyframe.value
            shapeKey.keyframe_insert(data_path="value", frame=keyframe.frame)
        

file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/test_note_overlap_1_11.mid")
test = file.findTrack("Test")

animator = MIDIAnimatorNode()
animator.addInstrument(instrumentType="custom", midiTrack=test, objectCollection=bpy.data.collections['NewSystem'], custom=NoteOffTesting)

# Animate the MIDI file
animator.animate()