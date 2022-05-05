import bpy


class TEST_PT_my_panel(bpy.types.Panel):
    bl_idname = "TEST_PT_my_panel"
    bl_label = "Test Panel"
    bl_category = "MIDIAnimator"
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"

    def draw(self, context):
        layout = self.layout

        row = layout.row()
        row.operator('view3d.test', text="Test Button")