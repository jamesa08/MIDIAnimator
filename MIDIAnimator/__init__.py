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

bl_info = {
    "name": "MIDI Animator",
    "description": "Animate objects with MIDI.",
    "author": "James Alt",
    "version": (0, 0, 1),
    "blender": (2, 91, 0),
    "location": "Scripting Space",
    "doc_url": "https://midianimatordocs.readthedocs.io/en/latest/",
    "tracker_url": "https://github.com/imacj/MIDIAnimator/issues",
    "warning": "",
    "support": "TESTING",
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
    from . ui import *
    from . ui.operators import SCENE_OT_quick_add_props
    from . ui.panels import VIEW3D_PT_edit_note_information, VIEW3D_PT_edit_instrument_information, VIEW3D_PT_add_notes_quick


classes = (SCENE_OT_quick_add_props, VIEW3D_PT_edit_instrument_information, VIEW3D_PT_edit_note_information, VIEW3D_PT_add_notes_quick)


def register():
    # Edit Note Information
    bpy.types.Object.note_number = bpy.props.StringProperty(
        name="Note Number", 
        description="The note number of an object. Can be entered as a integer (MIDI Note Number, e.g. 60) or as a "
                    "readable note (C3).",
        default="C3",
        options=set()
    )
    bpy.types.Object.animation_curve = bpy.types.Scene.quick_obj_curve = bpy.props.PointerProperty(
        name="Animation Curve", 
        description="The FCurve object to be read.",
        type=bpy.types.Object,
        options=set()
    )
    bpy.types.Object.animation_curve_index = bpy.types.Scene.quick_obj_curve_index = bpy.props.IntProperty(
        name="Animation Curve Index",
        description="The FCurve index (which curve in the FCurve to use).",
        default=0,
        options=set()
    )
    bpy.types.Object.note_hit_time = bpy.types.Scene.quick_note_hit_time = bpy.props.IntProperty(
        name="Note Hit Time",
        description="The time in which the ball hits",
        default=0,
        options=set()
    )

    # Instrument Types
    bpy.types.Collection.instrument_type = bpy.types.Scene.quick_instrument_type = bpy.props.EnumProperty(
        items=[
            ("projectile", "Projectile", ""), 
            ("string", "String", ""),
            ("custom", "Custom", "")
            ], 
        name="Instrument Type", 
        default="string",
        options=set()
    )
    bpy.types.Collection.projectile_collection = bpy.props.PointerProperty(
        name="Projectile Collection",
        description="This is where the projectiles will be stored",
        type=bpy.types.Collection,
        options=set()
    )
    bpy.types.Collection.reference_projectile = bpy.props.PointerProperty(
        name="Reference Projectile",
        description="The projectile object to be duplicated",
        type=bpy.types.Object,
        options=set()
    )
    bpy.types.Object.projectile_id = bpy.props.StringProperty(
        name="Projectile ID",
        description="",
        default="", 
        options=set()
    )

    # Edit Notes (Quick)
    bpy.types.Scene.note_number_list = bpy.props.StringProperty(
        name="Note Number List",
        description="A list of note numbers. These will correspond to the objects in the selected collection.",
        default="[]",
        options=set()
    )
    bpy.types.Scene.quick_use_sorted = bpy.props.BoolProperty(
        name="Quick Sort objects",
        description="This will use a sorted list of objects by name instead of using `name_noteNumber`",
        default=False,
        options=set()
    )
    bpy.types.Scene.quick_obj_col = bpy.props.PointerProperty(type=bpy.types.Collection, name="Collection to use")
    # bpy.types.Scene.quick_obj_curve = bpy.props.PointerProperty(type=bpy.types.Object, name="Anim Curve")
    # bpy.types.Scene.quick_obj_curve_index = bpy.props.IntProperty(name="Anim Curve Index", default=0)
    # bpy.types.Scene.quick_note_hit_time = bpy.props.IntProperty(name="Note Hit Time", default=0)

    
    for bpyClass in classes:
        bpy.utils.register_class(bpyClass)

def unregister():
    del bpy.types.Object.note_number
    del bpy.types.Object.animation_curve
    del bpy.types.Object.animation_curve_index
    del bpy.types.Object.note_hit_time
    del bpy.types.Collection.instrument_type

    del bpy.types.Collection.projectile_collection
    del bpy.types.Collection.reference_projectile
    del bpy.types.Object.projectile_id

    del bpy.types.Scene.quick_instrument_type
    del bpy.types.Scene.quick_obj_col
    del bpy.types.Scene.quick_obj_curve
    del bpy.types.Scene.quick_obj_curve_index
    del bpy.types.Scene.quick_note_hit_time
    del bpy.types.Scene.quick_use_sorted

    for bpyClass in classes:
        bpy.utils.unregister_class(bpyClass)