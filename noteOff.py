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
    try:
        pX = (((x1*y2)-(y1*x2)) * (x3-x4) - (x1-x2) * ((x3*y4)-(y3*x4))) / (((x1-x2) * (y3-y4)) - ((y1-y2) * (x3 - x4)))
        pY = (((x1*y2)-(y1*x2)) * (y3-y4) - (y1-y2) * ((x3*y4)-(y3*x4))) / (((x1-x2) * (y3-y4)) - ((y1-y2) * (x3 - x4)))
    except ZeroDivisionError:
        return 0, 0
    
    return pX, pY

def findInterval(frame, alreadyInsertedKeyframes):
    if len(alreadyInsertedKeyframes) < 2: return
    if frame < alreadyInsertedKeyframes[0].frame: return
    if frame > alreadyInsertedKeyframes[-1].frame: return

    out = []
    for i, insertedKey in enumerate(alreadyInsertedKeyframes):
        
        # if insertedKey.frame <= nextKeys[0].frame: continue
        if insertedKey.frame <= frame: continue

        # for nextKey in nextKeys:
        if frame <= insertedKey.frame:
            out.append((alreadyInsertedKeyframes[i-1], i-1))
            out.append((alreadyInsertedKeyframes[i], i))
            break
            
    return out


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
                bObjs = self.noteToBlenderObject[note.noteNumber]
            else:
                # TODO warn
                continue
            
            for bObj in bObjs:
                nextKeys = []
                # note on
                if bObj.obj.midi.note_on_curve:
                    for shapeName in bObj.noteOnCurves.shapeKeysDict:
                        fCrv = bObj.noteOnCurves.shapeKeysDict[shapeName][0]  # reference shape key
                        shapeKey = bObj.noteOnCurves.shapeKeysDict[shapeName][1]  # bObj shape key
                        for keyframe in fCrv.keyframe_points:
                            frame = keyframe.co[0] + secToFrames(note.timeOn) + bObj.obj.midi.note_on_anchor_pt
                            value = keyframe.co[1]
                            nextKeys.append(Keyframe(frame, value))
                
                if bObj.obj.midi.note_off_curve:
                    # note off
                    for shapeName in bObj.noteOffCurves.shapeKeysDict:
                        fCrv = bObj.noteOffCurves.shapeKeysDict[shapeName][0]  # reference shape key
                        shapeKey = bObj.noteOffCurves.shapeKeysDict[shapeName][1]  # bObj shape key
                        for keyframe in fCrv.keyframe_points:
                            frame = keyframe.co[0] + secToFrames(note.timeOff) + bObj.obj.midi.note_off_anchor_pt
                            value = keyframe.co[1]
                            nextKeys.append(Keyframe(frame,value))
                
                # keysToUpdate = []    
                # for nextKey in nextKeys:
                #     keyRange = findInterval(nextKey.frame, alreadyInserted)
                #     # print(nextKey.frame, keyRange)
                #     if keyRange is None: continue
                #     keysToUpdate.extend(keyRange)    
                    
                #     inMin, outMin = keyRange[0][0].frame, keyRange[1][0].value
                #     inMax, outMax = keyRange[1][0].frame, keyRange[1][0].value
                #     evaluatedCurveVal = mapRangeSin(nextKey.frame, inMin, inMax, outMin, outMax)
                #     nextKey.value += evaluatedCurveVal

                # for key, i in keysToUpdate:
                #     keyRange = findInterval(key.frame, nextKeys)
                #     if keyRange is None: continue
                    
                #     inMin, outMin = keyRange[0][0].frame, keyRange[1][0].value
                #     inMax, outMax = keyRange[1][0].frame, keyRange[1][0].value
                #     evaluatedCurveVal = mapRangeSin(key.frame, inMin, inMax, outMin, outMax)
                #     alreadyInserted[i].value += evaluatedCurveVal
                
                alreadyInserted.extend(nextKeys)
        
        
        for keyframe in sorted(alreadyInserted, key=lambda key: key.frame):
            print(f"{keyframe.frame},{keyframe.value}")
            shapeKey.value = keyframe.value
            shapeKey.keyframe_insert(data_path="value", frame=keyframe.frame)
                        

file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/test_midi_2notes_fixed.mid")
vibe = file.findTrack("Test")

animator = MIDIAnimatorNode()
animator.addInstrument(instrumentType="custom", midiTrack=vibe, objectCollection=bpy.data.collections['Cubes'], custom=NoteOffTesting)

# Animate the MIDI file
animator.animate()


# for key in shapeKeyFCurvesFromObject(bpy.data.objects['Cubes'])[0].keyframe_points:
#     print(f"{key.co[0]},{key.co[1]}")
