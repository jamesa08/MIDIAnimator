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
        scene = context.scene

        col = layout.column(heading="Add Property")

        col.prop(obj, "note_number")
        col.prop(obj, "animation_curve")
        col.prop(obj, "animation_curve_index")


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

        obj = context.active_object
        scene = context.scene

        col = layout.column()

        col.prop(scene, "note_number_list")
        col.prop(scene, "quick_obj_col_prop")
        col.prop(scene, "quick_obj_curve_prop")
        col.prop(scene, "quick_obj_curve_index_prop")
        col.operator("scene.quick_add_props")
        col.separator_spacer()