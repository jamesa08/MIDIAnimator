import imp


if "bpy" in locals():
    import importlib
    importlib.reload(functions)
    importlib.reload(MIDINode)
    importlib.reload(Note)
else:
    from .. utils import functions
    from . import MIDIFile
    from . import Note

import bpy