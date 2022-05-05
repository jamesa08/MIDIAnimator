if "bpy" in locals():
    import importlib
    importlib.reload(functions)
else:
    from . import functions
