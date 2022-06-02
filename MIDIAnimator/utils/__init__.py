if "bpy" in locals():
    import importlib
    importlib.reload(functions)
    importlib.reload(animation)
    importlib.reload(algorithms)
else:
    from . import functions
    from . import animation
    from . import algorithms
