if "bpy" in locals():
    import importlib
    importlib.reload(test_op)
    importlib.reload(test_panel)
else:
    from . import test_op
    from . import test_panel
