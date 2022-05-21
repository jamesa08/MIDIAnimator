if "bpy" in locals():
    import importlib
    importlib.reload(functions)
    importlib.reload(animation)
else:
    from . import functions
    from . import animation
