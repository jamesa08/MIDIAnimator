from __future__ import annotations
from .. src.instruments import *
import bpy

class MIDIAniamtorPanel:
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "MIDIAnimator"

class VIEW3D_PT_edit_instrument_information(MIDIAniamtorPanel, bpy.types.Panel):
    bl_label = "Edit Instrument Information"

    @classmethod
    def poll(cls, context):
        selectedObjs = context.selected_editable_objects
        # return True if collection is selected in outliner
        return len(selectedObjs) == 0


    def draw(self, context):
        if len(context.selected_editable_objects) != 0:
            blCol = context.object.users_collection[0]
        else:
            # fallback if empty collection
            blCol = context.collection
        
        blColMidi = blCol.midi

        layout = self.layout
        layout.use_property_decorate = False
        layout.use_property_split = True
        col = layout.column()
        
        col.label(text=f"Active collection: '{blCol.name}'")
        col.prop(blColMidi, "instrument_type", text="Instrument Type")

        for item in Instruments:
            value = item.value
            if value.identifier == blColMidi.instrument_type:
                value.cls.drawInstrument(context, col, blCol)
                break


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
        blColMidi = blCol.midi

        layout = self.layout
        layout.use_property_decorate = False
        layout.use_property_split = True

        col = layout.column()
        
        col.label(text=f"Active object: '{obj.name}'")

        
        for item in Instruments:
            value = item.value
            if value.identifier == blColMidi.instrument_type:
                value.cls.drawObject(context, col, obj)
                # if the property exists (precheck for next cond.) and is false or if the property does not exist at all, draw the note number object
                if (hasattr(value.cls, "EXCLUDE_NOTE_NUMBER") and not value.cls.EXCLUDE_NOTE_NUMBER) or not hasattr(value.cls, "EXCLUDE_NOTE_NUMBER"):
                    col.prop(objMidi, "note_number")
                break
        

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
