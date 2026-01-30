# This program is free software; you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation; either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful, but
# WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
# General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program. If not, see <http://www.gnu.org/licenses/>.

from __future__ import annotations
from . ui import VIEW3D_PT_server_link, SCENE_OT_connect_to_server, SCENE_OT_disconnect_from_server
from . src.tracker import SCENE_OT_StartSceneTracker, SCENE_OT_StopSceneTracker
from . src.core import Server
import bpy

bl_info = {
    "name": "MIDIAnimator Bridge beta5.0",
    "description": "Bridge between MIDIAnimator and Blender",
    "author": "James Alt (et al.)",
    "version": (0, 5, 0),
    "blender": (3, 0, 0),
    "location": "Scripting Space",
    "doc_url": "https://midianimatordocs.readthedocs.io/en/latest/",
    "tracker_url": "https://github.com/jamesa08/MIDIAnimator/issues",
    "warning": "MIDIAnimator is currently in beta. If you encounter any issues, please feel free to open an issue on GitHub (https://github.com/jamesa08/MIDIAnimator/issues)",
    "support": "COMMUNITY",
    "category": "Animation"
}

classes = (VIEW3D_PT_server_link, SCENE_OT_connect_to_server, SCENE_OT_disconnect_from_server, SCENE_OT_StartSceneTracker, SCENE_OT_StopSceneTracker)

# verify singleton
s1 = Server()
s2 = Server()

if id(s1) == id(s2):
    print(f"MIDIAnimator Bridge: verified singleton, debug id: {id(s1)}")
else:
    raise RuntimeError("MIDIAnimator Bridge: failed to verify singleton. Please open an isuse on GitHub.")

def register():
    for bpyClass in classes:
        bpy.utils.register_class(bpyClass)
        

def unregister():
    for bpyClass in classes:
        bpy.utils.unregister_class(bpyClass)

    
    # close the client connection when the addon is unregistered
    client = Server()
    client.close()
