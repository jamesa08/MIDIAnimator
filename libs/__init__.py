if "bpy" in locals():
    import importlib
    importlib.reload(mido)
else:
    from . import mido