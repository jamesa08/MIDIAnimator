import bpy

class TEST_OT_my_operator(bpy.types.Operator):
    bl_idname = "view3d.test"
    bl_label = "Simple"
    bl_description = "Prints 'Test' to the console."

    def execute(self, context):
        print("Test")
        return {'FINISHED'}