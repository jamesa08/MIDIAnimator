if "bpy" in locals():
    import importlib
    importlib.reload(functions)
    importlib.reload(MIDINode)
else:
    from .. utils import functions
    from . import MIDINode

import bpy