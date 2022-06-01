import bpy

class MIDIAniamtorPanel:
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "MIDIAnimator"
    bl_options = {"DEFAULT_CLOSED"}

class VIEW3D_PT_edit_note_information(MIDIAniamtorPanel, bpy.types.Panel):
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "MIDIAnimator"
    bl_label = "Edit Note Information"

    def draw(self, context):
        layout = self.layout
        layout.use_property_decorate = False
        layout.use_property_split = True

        obj = context.active_object
        objs = context.selected_editable_objects
        blCol = context.collection

        col = layout.column()

        if blCol != [] and objs == []:
            # collection is selected
            col.prop(blCol, "instrument_type", text="Instrument Type")

        if objs != []:
            # object is selected
            col.prop(obj, "note_number")

            row = col.row()
            row.prop(obj, "animation_curve")
            row.prop(obj, "animation_curve_index", text="")
            
            col.prop(obj, "note_hit_time")


class VIEW3D_PT_add_notes_quick(MIDIAniamtorPanel, bpy.types.Panel):
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "MIDIAnimator"
    bl_parent_id = "VIEW3D_PT_edit_note_information"
    bl_label = "Edit Notes (Quick)"

    def draw(self, context):
        layout = self.layout
        layout.use_property_decorate = False
        layout.use_property_split = True

        scene = context.scene
        if scene is not None:

            col = layout.column()
            col.prop(scene, "quick_instrument_type")
            col.prop(scene, "note_number_list")
            col.prop(scene, "quick_obj_col_prop")

            row = col.row()
            row.prop(scene, "quick_obj_curve_prop")
            row.prop(scene, "quick_obj_curve_index_prop", text="")
            
            col.prop(scene, "quick_note_hit_time")
            col.operator("scene.quick_add_props")
            col.separator_spacer()
        else:
            layout.label(text="Scene does not exist!")

