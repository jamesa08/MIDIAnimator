# This program is free software; you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation; either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful, but
# WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
# General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program. If not, see <http://www.gnu.org/licenses/>.

from __future__ import annotations

bl_info = {
    "name": "MIDI Animator beta3.4",
    "description": "A cohesive, open-source solution to animating Blender objects using a MIDI file.",
    "author": "James Alt (et al.)",
    "version": (1, 0, 0),
    "blender": (3, 0, 0),
    "location": "Scripting Space",
    "doc_url": "https://midianimatordocs.readthedocs.io/en/latest/",
    "tracker_url": "https://github.com/jamesa08/MIDIAnimator/issues",
    "warning": "MIDIAnimator is currently in beta. If you encounter any issues, please feel free to open an issue on GitHub (https://github.com/jamesa08/MIDIAnimator/issues)",
    "support": "COMMUNITY",
    "category": "Animation"
}

if "bpy" in locals():
    # Running under Blender
    import importlib
    importlib.reload(src)
    importlib.reload(utils)
    importlib.reload(ui)
else:
    # Running under external instance
    import bpy
    from . src import *
    from . utils import *
    from . utils.loggerSetup import *
    from . ui import *
    from . ui.operators import SCENE_OT_quick_add_props
    from . ui.panels import VIEW3D_PT_edit_object_information, VIEW3D_PT_add_notes_quick


class MIDIAnimatorObjectProperties(bpy.types.PropertyGroup):
    note_number:  bpy.props.StringProperty(
        name="Note Number", 
        description="The note number of an object. Can be entered as a integer (MIDI Note Number, e.g. 60) or as a "
                    "readable note (C3)",
        default="C3"
    )
    note_number_int:  bpy.props.IntProperty(
        name="Note Number Integer", 
        description="The note number (integer) of an object",
        default=60
    )
    note_on_curve: bpy.props.PointerProperty(
        name="Note On Animation Curve", 
        description="The animation curve object with defined keyframes to be read in",
        type=bpy.types.Object,
        options=set()
    )
    note_on_anchor_pt: bpy.props.IntProperty(
        name="Note On Anchor Point",
        description="Where along should we start animating (in reference to the note on time). 0 to start animating right on note on times, - for eariler, + for later",
        default=0,
        options=set()
    )
    note_off_curve: bpy.props.PointerProperty(
        name="Note Off Animation Curve", 
        description="The animation curve object with defined keyframes to be read in."
                    "\n\nDisabled: will be added in a future release",
        type=bpy.types.Object,
        options=set()
    )
    note_off_anchor_pt: bpy.props.IntProperty(
        name="Note Off Anchor Point",
        description="Where along should we start animating (in reference to the note on time). 0 to start animating right on note off times, - for eariler, + for later."
                    "\n\nDisabled: will be added in a future release",
        default=0,
        options=set()
    )
    osc_period: bpy.props.FloatProperty(
        name="Period",
        description="Period of the oscillation",
        default=4,
        options=set()
    )
    osc_amp: bpy.props.FloatProperty(
        name="Amplitude",
        description="Amplitude of the oscillation",
        default=4,
        options=set()
    )
    osc_damp: bpy.props.FloatProperty(
        name="Damping",
        description="Damping of the oscillation",
        default=10,
        options=set()
    )
    animation_overlap: bpy.props.EnumProperty(
        items=[
            ("add", "Add", "Curves will add motion. More options will be added in the future")
        ],
        name="Animation Overlap",
        default="add",
        options=set()
    )
    anim_type: bpy.props.EnumProperty(
        items=[
            ("keyframed", "Keyframed", "Pre-defined FCurve objects to refernce the animation from"),
            ("damp_osc", "Oscillation", "Dampened oscillation. Planned for a future release"),
            ("adsr", "ADSR", "Attack, Decay, Sustain, Release. Planned for a future release")
        ],
        name="Animation Type",
        description="Disabled: will be added in a future release",
        default="keyframed",
        options=set()
    )
    
class MIDIAnimatorSceneProperties(bpy.types.PropertyGroup):
    # Edit Notes (Quick)
    quick_note_number_list: bpy.props.StringProperty(
        name="Note Number List",
        description="A list of note numbers. These will correspond to the objects in the selected collection",
        default="[]",
        options=set()
    )
    quick_obj_col: bpy.props.PointerProperty(
        type=bpy.types.Collection, 
        name="Collection",
    )
    quick_sort_by_name: bpy.props.BoolProperty(
        name="Sort by Name",
        description="This will use a sorted list of objects by name instead of using `name_noteNumber`",
        default=False,
        options=set()
    )

classes = (SCENE_OT_quick_add_props, VIEW3D_PT_edit_object_information, VIEW3D_PT_add_notes_quick, MIDIAnimatorObjectProperties, MIDIAnimatorSceneProperties)

def register():
    for bpyClass in classes:
        bpy.utils.register_class(bpyClass)
    
    bpy.types.Object.midi = bpy.props.PointerProperty(type=MIDIAnimatorObjectProperties)
    bpy.types.Scene.midi = bpy.props.PointerProperty(type=MIDIAnimatorSceneProperties)

    logger.info("MIDIAnimator registered successfully")

def unregister():
    for bpyClass in classes:
        bpy.utils.unregister_class(bpyClass)

    del bpy.types.Object.midi
    del bpy.types.Scene.midi

    logger.info("MIDIAnimator unregistered successfully")