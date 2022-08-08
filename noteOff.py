import bpy
from MIDIAnimator.data_structures import FrameRange
from MIDIAnimator.src.animation import MIDIAnimatorNode
from MIDIAnimator.data_structures.midi import MIDIFile, MIDITrack, MIDINote
from MIDIAnimator.src.instruments import Instrument
from MIDIAnimator.utils.blender import *
from MIDIAnimator.utils import *
from dataclasses import dataclass

@dataclass
class Keyframe:
    frame: float
    value: float
    keyType: str = ""

def findIntersection(x1, y1, x2, y2, x3, y3, x4, y4):
    pX = (((x1*y2)-(y1*x2)) * (x3-x4) - (x1-x2) * ((x3*y4)-(y3*x4))) / (((x1-x2) * (y3-y4)) - ((y1-y2) * (x3 - x4)))
    pY = (((x1*y2)-(y1*x2)) * (y3-y4) - (y1-y2) * ((x3*y4)-(y3*x4))) / (((x1-x2) * (y3-y4)) - ((y1-y2) * (x3 - x4)))
    return pX, pY


class NoteOffTesting(Instrument):
    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, **kwargs):
        super().__init__(midiTrack, collection, override=True)
        self.override = True
        self.preAnimate()
        
        noteOnCurves = self.makeObjToFCurveDict(type="note_on")
        noteOffCurves = self.makeObjToFCurveDict(type="note_off")
        self.createNoteToBlenderObject(noteOnCurves, noteOffCurves)
        self.calculateFrameRanges()

    def preAnimate(self):
        bpy.context.scene.frame_set(-10000)
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)

    def calculateFrameRanges(self):
        out = []
        for note in self.midiTrack.notes:
            if note.noteNumber in self.noteToBlenderObject:
                bObjs = self.noteToBlenderObject[note.noteNumber]
            else:
                # warn
                continue

            for bObj in bObjs:
                obj = bObj.obj
                objMidi = obj.midi
                
                rangeOn, rangeOff = 0.0, 0.0
                
                if objMidi.anim_curve_type == "keyframed":

                    if objMidi.note_on_curve:
                        rangeOn = bObj.rangeOn()[1] - bObj.rangeOn()[0]
                        rangeOn += objMidi.note_on_anchor_pt
                        rangeOn += secToFrames(note.timeOn)
                    
                    if objMidi.note_off_curve:
                        rangeOff = bObj.rangeOff()[1] - bObj.rangeOff()[0]
                        rangeOff += objMidi.note_off_anchor_pt
                        rangeOff += secToFrames(note.timeOff)
                
                elif objMidi.anim_curve_type == "damp_osc":
                    pass
                
                elif objMidi.anim_curve_type == "adsr":
                    pass
                
                out.append(FrameRange(rangeOn, rangeOff, bObj))
        
        return out

    
    def animate(self):
        print("x, y")
        alreadyInserted = []
        for note in self.midiTrack.notes:
            if note.noteNumber in self.noteToBlenderObject:
                print()
                bObjs = self.noteToBlenderObject[note.noteNumber]
            else:
                # warn
                continue
            
            for bObj in bObjs:
                nextKeys = []
                # note on
                for shapeName in bObj.noteOnCurves.shapeKeysDict:
                    fCrv = bObj.noteOnCurves.shapeKeysDict[shapeName][0]  # reference shape key
                    shapeKey = bObj.noteOnCurves.shapeKeysDict[shapeName][1]  # bObj shape key
                    for keyframe in fCrv.keyframe_points:
                        frame = keyframe.co[0] + secToFrames(note.timeOn) + bObj.obj.midi.note_on_anchor_pt
                        value = keyframe.co[1]
                        # print(f"{frame},{value},{secToFrames(note.timeOn)}")
                        nextKeys.append(Keyframe(frame, value))
                        # shapeKey.value = value
                        # shapeKey.keyframe_insert(data_path="value", frame=frame)
                
                # note off
                for shapeName in bObj.noteOffCurves.shapeKeysDict:
                    fCrv = bObj.noteOffCurves.shapeKeysDict[shapeName][0]  # reference shape key
                    shapeKey = bObj.noteOffCurves.shapeKeysDict[shapeName][1]  # bObj shape key
                    for keyframe in fCrv.keyframe_points:
                        frame = keyframe.co[0] + secToFrames(note.timeOff) + bObj.obj.midi.note_off_anchor_pt
                        value = keyframe.co[1]
                        nextKeys.append(Keyframe(frame,value))
                        # print(f"{frame},{value},{secToFrames(note.timeOff)}")
                        
                        # shapeKey.value = value
                        # shapeKey.keyframe_insert(data_path="value", frame=frame)
                
                for i, key in enumerate(nextKeys.copy()):
                    if len(alreadyInserted) != 0:
                        lastKey = alreadyInserted[-1]
                    else:
                        lastKey = Keyframe(0,0)
                    print(key, lastKey)
                    if lastKey.frame > key.frame and lastKey.value != key.value:
                        # print("overlap detected", lastKey.frame, keyframe.frame)
                        # calculate slope w/ point slope formula
                        x1, y1 = alreadyInserted[-2].frame, alreadyInserted[-2].value
                        x2, y2 = alreadyInserted[-1].frame, alreadyInserted[-1].value
                        x3, y3 = nextKeys[i-1].frame, nextKeys[i-1].value
                        x4, y4 = nextKeys[i].frame, nextKeys[i].value
                        pX, pY = findIntersection(x1, y1, x2, y2, x3, y3, x4, y4)
                        # delete offending keyframes
                        nextKeys.pop(0)
                        alreadyInserted.pop(-1)
                        
                        alreadyInserted.append(Keyframe(pX, pY))
                
                alreadyInserted.extend(nextKeys)
        

        for keyframe in alreadyInserted:
            shapeKey.value = keyframe.value
            shapeKey.keyframe_insert(data_path="value", frame=keyframe.frame)
                        

file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/pd1_vibe.mid")
vibe = file.findTrack("Vibraphone")

animator = MIDIAnimatorNode()
animator.addInstrument(instrumentType="custom", midiTrack=vibe, objectCollection=bpy.data.collections['Cubes'], custom=NoteOffTesting)

# Animate the MIDI file
animator.animate()