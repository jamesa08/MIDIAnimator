from MIDIAnimator.src.animation import *
from MIDIAnimator.utils.blender import *
from typing import Callable, List, Tuple, Dict, Optional, Union
from MIDIAnimator.data_structures.midi import MIDIFile
import bpy

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
drumNotes = [36, 36, 38, 41, 43, 45, 47, 48, 49, 50, 55, 56, 57, 59, 60]  # DOES NOT INCLUDE HIHAT
drumHH = [42, 44, 46]
drum4Way = drumHH[:] + [55, 56, 59, 60]
drum4Way.remove(44)  # remove pedal hh

file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/pipedream3_8_18_21_1.mid")
drumTrack = file.findTrack("Drums")
animator = MIDIAnimatorNode()

scene = bpy.context.scene

# ------------------------- assign notes to drums -------------------------
# scene.quick_instrument_type = "string"
# scene["note_number_list"] = str(drumNotes)
# scene["quick_obj_col"] = bpy.data.collections['DrumAnimationObjects']
# scene["quick_obj_curve"] = bpy.context.scene.objects['ANIM_DrumBounce']
# scene["quick_note_hit_time"] = 0
# scene["quick_use_sorted"] = False
# bpy.ops.scene.quick_add_props()

# # update other objects
# bpy.data.objects['cymbal_1_49'].animation_curve = bpy.data.objects['ANIM_Cyms']
# bpy.data.objects['cymbal_2_57'].animation_curve = bpy.data.objects['ANIM_Cyms']

# bpy.data.objects['splash_gimbal_55'].animation_curve = bpy.data.objects['ANIM_Splash']

# bpy.data.objects['hi_woodblock_59'].animation_curve = bpy.data.objects['ANIM_WoodblockBounce']
# bpy.data.objects['lo_woodblock_60'].animation_curve = bpy.data.objects['ANIM_WoodblockBounce']

# bpy.data.objects['cowbell_56'].animation_curve = bpy.data.objects['ANIM_Cowbell']


# ------------------------- define projectiles -------------------------
refBall = scene.objects['REF_Ball']

drumProjTable = {
    # "instrument": ["fake funnel col name", "where the balls are going to end up name", "note number", "ball animation name", "hit time"]
    "bass": ['bass funnel', 'bass balls', 36, "ANIM_bassball", 22],
    "snare": ['snare funnel', 'snare balls', 38, "ANIM_snareBall", 20],
    "crash 1": ['crash1 funnel', 'crash1 balls', 49, "ANIM_crash1Ball", 20],
    "crash 2": ['crash2 funnel', 'crash2 balls', 57, "ANIM_crash2Ball", 20],
    "tom1": ['tom1 funnel', 'tom1 balls', 50, "ANIM_tom1Ball", 20],
    "tom2": ['tom2 funnel', 'tom2 balls', 48, "ANIM_tom2Ball", 20],
    "tom3": ['tom3 funnel', 'tom3 balls', 47, "ANIM_tom3Ball", 20],
    "tom4": ['tom4 funnel', 'tom4 balls', 45, "ANIM_tom4Ball", 20],
    "tom5": ['tom5 funnel', 'tom5 balls', 43, "ANIM_tom5Ball", 20],
    "tom6": ['tom6 funnel', 'tom6 balls', 41, "ANIM_tom6Ball", 20],
    "splash": ['splash funnel', 'splash balls', 55, "ANIM_4WayPercSplashBall", 20],
    "hh open": ['hh open funnel', 'hh open balls', 46, "ANIM_4WayPercHHOpenBall", 20],
    "hh closed": ['hh closed funnel', 'hh closed balls', 42, "ANIM_4WayPercHHClosedBall", 20],
    "woodblock lo": ['wb lo funnel', 'wb lo balls', 60, "ANIM_4WayPercWBLowBall", 20],
    "woodblock hi": ['wb hi funnel', 'wb hi balls', 59, "ANIM_4WayPercWBHiBall", 20],
    "cowbell": ['cowbell funnel', 'cowbell balls', 56, "ANIM_4WayPercCowbellBall", 20]
}

for key in drumProjTable:
    fakeFunnelName, ballColName, noteNumber, ballAnimName, hitTime = drumProjTable[key]

    scene.quick_instrument_type = "projectile"

    # set up projectile collection (Edit Instrument Information)
    bpy.data.collections[fakeFunnelName].instrument_type = "projectile"
    bpy.data.collections[fakeFunnelName]['projectile_collection'] = bpy.data.collections[ballColName]
    bpy.data.collections[fakeFunnelName]['reference_projectile'] = refBall

    # set up props menu
    scene["note_number_list"] = str([noteNumber])
    scene["quick_obj_col"] = bpy.data.collections[fakeFunnelName]
    scene["quick_obj_curve"] = scene.objects[ballAnimName]
    scene["quick_use_sorted"] = True
    scene["quick_note_hit_time"] = hitTime
    bpy.ops.scene.quick_add_props()
    
    # add the track
    # animator.addInstrument(midiTrack=drumTrack, objectCollection=bpy.data.collections[fakeFunnelName])


# add bounce instruments
animator.addInstrument(midiTrack=drumTrack, objectCollection=bpy.data.collections['DrumAnimationObjects'])

animator.animate()