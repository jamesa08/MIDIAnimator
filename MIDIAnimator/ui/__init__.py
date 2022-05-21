if "bpy" in locals():
    import importlib
    importlib.reload(operators)
    importlib.reload(panels)
else:
    from . import operators
    from . import panels
