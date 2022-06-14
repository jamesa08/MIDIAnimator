if "bpy" in locals():
    import importlib
    importlib.reload(MIDIStructure)
    importlib.reload(animation)
else:
    from . import MIDIStructure
    from . import animation

import bpy