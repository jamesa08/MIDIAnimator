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
    "name": "MIDI Animator beta4.0",
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
    from . src.instruments import Instruments, MIDIAnimatorObjectProperties, MIDIAnimatorCollectionProperties, MIDIAnimatorSceneProperties
    from . utils import *
    from . utils.loggerSetup import *
    from . ui import *
    from . ui.operators import SCENE_OT_quick_add_props
    from . ui.panels import VIEW3D_PT_edit_instrument_information, VIEW3D_PT_edit_object_information, VIEW3D_PT_add_notes_quick


classes = (SCENE_OT_quick_add_props, VIEW3D_PT_edit_instrument_information, VIEW3D_PT_edit_object_information, VIEW3D_PT_add_notes_quick, MIDIAnimatorObjectProperties, MIDIAnimatorCollectionProperties, MIDIAnimatorSceneProperties)

def register():
    for bpyClass in classes:
        bpy.utils.register_class(bpyClass)

        
    bpy.types.Object.midi = bpy.props.PointerProperty(type=MIDIAnimatorObjectProperties)
    bpy.types.Collection.midi = bpy.props.PointerProperty(type=MIDIAnimatorCollectionProperties)
    bpy.types.Scene.midi = bpy.props.PointerProperty(type=MIDIAnimatorSceneProperties)
    
    for item in Instruments:
        # register all Instrument properties
        item.value.cls.properties()

    logger.info("MIDIAnimator registered successfully")

def unregister():
    for bpyClass in classes:
        bpy.utils.unregister_class(bpyClass)

    del bpy.types.Object.midi
    del bpy.types.Collection.midi
    del bpy.types.Scene.midi

    logger.info("MIDIAnimator unregistered successfully")