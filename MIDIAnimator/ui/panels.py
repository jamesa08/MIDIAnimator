from __future__ import annotations
from .. utils import convertNoteNumbers, typeOfNoteNumber, noteToName
from ast import literal_eval
import bpy

class MIDIAniamtorPanel:
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "MIDIAnimator"

class VIEW3D_PT_edit_object_information(MIDIAniamtorPanel, bpy.types.Panel):
    bl_label = "Edit Object Information"

    @classmethod
    def poll(cls, context):
        selectedObjs = context.selected_editable_objects
        # return if object is selected
        return len(selectedObjs) != 0

    def draw(self, context):
        obj = context.active_object
        objMidi = obj.midi
        blCol = obj.users_collection[0]

        layout = self.layout
        layout.use_property_decorate = False
        layout.use_property_split = True

        col = layout.column()
        
        col.label(text=f"Active object: '{obj.name}'")
        col.prop(objMidi, "note_number")
        
        col.separator()
        row0 = col.row()

        row0.prop(objMidi, "anim_type")

        if objMidi.anim_type == "damp_osc":
            col.prop(objMidi, "osc_period")
            col.prop(objMidi, "osc_amp")
            col.prop(objMidi, "osc_damp")

        elif objMidi.anim_type == "keyframed":
            row1 = col.row()
            row1.prop(objMidi, "note_on_curve", text="Note On")
            # this is temporary until I determine a solution for each FCurve
            row1.prop(objMidi, "note_on_anchor_pt", text="")

            row2 = col.row()
            row2.prop(objMidi, "note_off_curve", text="Note Off")
            row2.prop(objMidi, "note_off_anchor_pt", text="")

        elif objMidi.anim_type == "adsr":
            col.label(text="Coming soon")
        
        col.separator()

        col.prop(objMidi, "anim_overlap")
        

class VIEW3D_PT_add_notes_quick(MIDIAniamtorPanel, bpy.types.Panel):
    bl_label = "Assign Notes to Objects"

    def draw(self, context):
        layout = self.layout
        layout.use_property_decorate = False
        layout.use_property_split = True
        col = layout.column()
        
        scene = context.scene
        sceneMidi = scene.midi

        col.prop(sceneMidi, "quick_note_number_list")
        col.prop(sceneMidi, "quick_obj_col")
        col.prop(sceneMidi, "quick_sort_by_name")
        
        col.separator_spacer()
        col.operator("scene.quick_add_props", text="Run")
        col.separator_spacer()
