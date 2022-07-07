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
        # print(context.collection, context.object.users_collection)
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
        
        elif blCol.instrument_type == "string":
            col.label(text="Coming soon.")

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
        
        if blCol.instrument_type == "projectile":
            col.prop(obj, "note_hit_time")

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

        col.prop(scene, "quick_instrument_type")
        col.prop(scene, "note_number_list")
        col.prop(scene, "quick_obj_col")

        col.prop(scene, "quick_obj_curve")
        
        col.prop(scene, "quick_use_sorted")
        
        if scene.quick_instrument_type == "projectile":
            col.separator_spacer()
            col.prop(scene, "quick_note_hit_time")
        elif scene.quick_instrument_type == "string":
            pass

        col.separator_spacer()
        col.operator("scene.quick_add_props", text="Run")
        col.separator_spacer()
