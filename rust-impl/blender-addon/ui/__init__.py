import bpy
from .. src.core import Server

class SCENE_OT_connect_to_server(bpy.types.Operator):
    bl_idname = "scene.connect_to_server"
    bl_label = "Connect to Server"
    bl_description = "Connect to the server at port 6577"

    def ping(self):
        pass

    def execute(self, context):
        client = Server()
        res = client.open()
        if not res:
            self.report({"ERROR"}, "Could not connect to server. Make sure MIDIAnimator is running.")
        return {"FINISHED"}
    
class SCENE_OT_disconnect_from_server(bpy.types.Operator):
    bl_idname = "scene.disconnect_from_server"
    bl_label = "Disconnect from Server"
    bl_description = "Disconnect from the server at port 6577"

    def execute(self, context):
        client = Server()
        client.close()
        return {"FINISHED"}

class MIDIAniamtorPanel:
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "MIDIAnimator Link"

class VIEW3D_PT_server_link(MIDIAniamtorPanel, bpy.types.Panel):
    bl_label = "MIDIAnimator Link"

    @classmethod
    def poll(cls, context):
        return True

    def draw(self, context):
        client = Server()

        layout = self.layout
        layout.use_property_decorate = False
        layout.use_property_split = True
        
        col = layout.column()
        col.label(text="Client at port 6577")

        if client.connected:
            col.label(text="Connected to Server")
            col.operator("scene.disconnect_from_server", text="Disconnect")
        else:
            col.label(text="Not connected to Server")
            col.operator("scene.connect_to_server", text="Connect")
        # if context.scene.connect_to_server == True:
            # col.label(text="Connected to Server")
        # col.prop(context.scene, "server_link", text="Server Link")

