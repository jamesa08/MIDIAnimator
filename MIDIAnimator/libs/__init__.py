# credits:
# local copy of mido library used here
# https://github.com/mido/mido

if "bpy" in locals():
    import importlib
    importlib.reload(mido)
else:
    from . import mido