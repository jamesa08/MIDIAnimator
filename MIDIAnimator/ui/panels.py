import bpy

class MIDIAniamtorPanel:
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "MIDIAnimator"
    bl_options = {"DEFAULT_CLOSED"}

class VIEW3D_PT_edit_instrument_information(MIDIAniamtorPanel, bpy.types.Panel):
    bl_label = "Edit Instrument Information"

    @classmethod
    def poll(cls, context):
        selectedObjs = context.selected_editable_objects
        blCol = context.collection
        
        # return if collection is selected in outliner
        return (len(blCol.all_objects) != 0 and len(selectedObjs) == 0) or len(context.object.users_collection) == 1


    def draw(self, context):
        selectedObjs = context.selected_editable_objects

        if (len(context.collection.all_objects) != 0 and len(selectedObjs) == 0):
            blCol = context.collection
        elif len(selectedObjs) != 0:
            blCol = context.object.users_collection[0]
        else:
            # fallback if empty collection
            blCol = context.collection

        layout = self.layout
        layout.use_property_decorate = False
        layout.use_property_split = True
        col = layout.column()
        
        col.label(text=f"Active collection: '{blCol.name}'")
        col.prop(blCol, "instrument_type", text="Instrument Type")

        if blCol.instrument_type == "projectile":
            col.prop(blCol, "projectile_collection")

            if blCol.projectile_collection is not None:
                col.prop(blCol, "reference_projectile")  # TODO: can there be a way to filter based on projectile_collection?


class VIEW3D_PT_edit_note_information(MIDIAniamtorPanel, bpy.types.Panel):
    bl_label = "Edit Note Information"

    @classmethod
    def poll(cls, context):
        selectedObjs = context.selected_editable_objects
        # return if object is selected
        return len(selectedObjs) != 0

    def draw(self, context):
        obj = context.active_object
        blCol = obj.users_collection[0]

        layout = self.layout
        layout.use_property_decorate = False
        layout.use_property_split = True

        col = layout.column()
        
        col.label(text=f"Active object: '{obj.name}'")
        col.prop(obj, "note_number")

        col.prop(obj, "animation_curve")
        
        col.prop(obj, "note_hit_time")

class VIEW3D_PT_add_notes_quick(MIDIAniamtorPanel, bpy.types.Panel):
    bl_label = "Assign Notes to Objects"

    def draw(self, context):
        layout = self.layout
        layout.use_property_decorate = False
        layout.use_property_split = True
        col = layout.column()
        
        scene = context.scene

        col.prop(scene, "quick_instrument_type")
        col.prop(scene, "note_number_list")
        col.prop(scene, "quick_obj_col")

        col.prop(scene, "quick_obj_curve")
        
        col.prop(scene, "quick_use_sorted")
        
        col.prop(scene, "quick_note_hit_time")

        col.separator_spacer()
        col.operator("scene.quick_add_props", text="Run")
        col.separator_spacer()
