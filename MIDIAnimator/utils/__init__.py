if "bpy" in locals():
    import importlib
    importlib.reload(functions)
    importlib.reload(blender)
    importlib.reload(algorithms)
else:
    from . import functions
    from . import blender
    from . import algorithms
