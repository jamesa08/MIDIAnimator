if "bpy" in locals():
    import importlib
    importlib.reload(midi)
    importlib.reload(animation)
else:
    from ..data_structures import midi
    from . import animation

import bpy