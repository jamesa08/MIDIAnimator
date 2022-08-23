from ast import literal_eval
import bpy
import gpu
import bgl
from gpu_extras.batch import batch_for_shader

from MIDIAnimator.utils import convertNoteNumbers, typeOfNoteNumber, noteToName

class MIDIAniamtorPanel:
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "MIDIAnimator"
    # bl_options = {"DEFAULT_CLOSED"}

# class VIEW3D_PT_edit_instrument_information(MIDIAniamtorPanel, bpy.types.Panel):
#     bl_label = "Edit Instrument Information"

#     @classmethod
#     def poll(cls, context):
#         selectedObjs = context.selected_editable_objects
#         blCol = context.collection
#         # return if collection is selected in outliner
#         return (len(blCol.all_objects) != 0 and len(selectedObjs) == 0) or len(context.object.users_collection) == 1


#     def draw(self, context):
#         # print(context.collection, context.object.users_collection)
#         selectedObjs = context.selected_editable_objects

#         if (len(context.collection.all_objects) != 0 and len(selectedObjs) == 0):
#             blCol = context.collection
#         elif len(selectedObjs) != 0:
#             blCol = context.object.users_collection[0]
#         else:
#             # fallback if empty collection
#             blCol = context.collection
        
#         blColMidi = blCol.midi

#         layout = self.layout
#         layout.use_property_decorate = False
#         layout.use_property_split = True
#         col = layout.column()
        
#         col.label(text=f"Active collection: '{blCol.name}'")
#         col.prop(blColMidi, "instrument_type", text="Instrument Type")

#         if blColMidi.instrument_type == "projectile":
#             col.prop(blColMidi, "projectile_collection")

#             if blColMidi.projectile_collection is not None:
#                 col.prop(blColMidi, "reference_projectile")  # TODO: can there be a way to filter based on projectile_collection?

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
        
        col.prop(objMidi, "anim_curve_type", text="Animation Curve Type")
        
        if objMidi.anim_curve_type == "damp_osc":
            col.prop(objMidi, "osc_period")
            col.prop(objMidi, "osc_amp")
            col.prop(objMidi, "osc_damp")

        elif objMidi.anim_curve_type == "keyframed":
            row1 = col.row()
            row1.prop(objMidi, "note_on_curve", text="Note On")
            # this is temporary until I determine a solution for each FCurve
            row1.prop(objMidi, "note_on_anchor_pt", text="")

            row2 = col.row()
            row2.prop(objMidi, "note_off_curve", text="Note Off")
            row2.prop(objMidi, "note_off_anchor_pt", text="")
            
        elif objMidi.anim_curve_type == "adsr":
            col.label(text="Coming soon")
        
        col.separator()

        col.prop(objMidi, "animation_overlap")
        
        # if blCol.midi.instrument_type == "projectile":
        #     col.prop(objMidi, "note_hit_time")

class VIEW3D_PT_add_notes_quick(MIDIAniamtorPanel, bpy.types.Panel):
    # bl_parent_id = "VIEW3D_PT_edit_note_information"
    bl_label = "Assign Notes to Objects"

    def draw(self, context):
        # TODO: > eventually have a way to see how the notes will be mapped (?)
        #       > for example if there are 3 objects in the collection to use
        #       > and a list is filled in, try to eval the list
        #       > and then show a preview of the mapping
        #       > 
        #       > Objects to be mapped: 
        #       >   object  ->   note
        #       > funnel_48 -> note 48/C2
        #       > funnel_50 -> note 50/D2
        #       > funnel_52 -> note 52/E2
        #       > 
        #       > or if the objects do not have the 'name_noteNumber' key,
        #       > they should be just in sorted order, for example:
        #       > 
        #       > Objects to be mapped: 
        #       >    object  ->    note
        #       > funnel     -> note 48/C2
        #       > funnel.001 -> note 50/D2
        #       > funnel.002 -> note 52/E2
        #       > 
        #       > this should be easy to implement with the blender UI
        #       > 
        #       > Q: should this be a warning w/OK btn when the bpy.context.scene.quick_add_props() is executed?
        #       >    and then have a pretty plaintext table?
        #       > 
        #       > still up in the air if this feature should really be implemented
        #       > but for now lets plan on it
    
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
