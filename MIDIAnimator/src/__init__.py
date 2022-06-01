if "bpy" in locals():
    import importlib
    importlib.reload(MIDIStructure)
    importlib.reload(Blender)
else:
    from . import MIDIStructure
    from . import animation

import bpy