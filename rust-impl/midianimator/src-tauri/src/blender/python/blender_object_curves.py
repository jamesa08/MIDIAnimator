import json
import bpy
from bpy_extras import anim_utils

# action slots came in Blender 4.4. from then on the old action.fcurves API only reaches a legacy slot the object isn't
# assigned to, so keys written through it never animate, and reading it can miss the slot the object uses
SLOTTED_ACTIONS = bpy.app.version >= (4, 4, 0)

# filled in by MotionKeys (blender/curves.rs): "watch" is every object the graph reads keyframes from, "fetch" the ones
# it needs curves for now, "limit" the most keyframes an object can have before it asks first, "approved" the ones it may
# send anyway
REQUEST = r""""""

def FCurvesFromObject(obj):
    """Gets FCurves from an object."""
    if obj.animation_data is None: return []
    if obj.animation_data.action is None: return []

    if not SLOTTED_ACTIONS:
        return list(obj.animation_data.action.fcurves)
    else:
        anim_data = obj.animation_data
        channelbag = anim_utils.action_get_channelbag_for_slot(anim_data.action, anim_data.action_slot)
        return list(channelbag.fcurves) if channelbag else []

def ShapeKeyFCurvesFromObject(obj):
    """Gets the FCurves of an object's shape keys, their data paths are like `key_blocks["Smile"].value`."""
    shape_keys = getattr(obj.data, "shape_keys", None)
    if shape_keys is None or shape_keys.animation_data is None: return []
    if shape_keys.animation_data.action is None: return []

    if not SLOTTED_ACTIONS:
        return list(shape_keys.animation_data.action.fcurves)
    else:
        anim_data = shape_keys.animation_data
        channelbag = anim_utils.action_get_channelbag_for_slot(anim_data.action, anim_data.action_slot)
        return list(channelbag.fcurves) if channelbag else []

def frames_to_sec(f, fps):
    return f / fps

def scene_fps(scene):
    """The scene's frame rate with its base (29.97 is 30 / 1.001), the same one the scene writer converts with."""
    return scene.render.fps / scene.render.fps_base

def get_fcurve_data(fcurve):
    """Converts an FCurve into a dictionary representation."""
    fps = scene_fps(bpy.context.scene)
    # times in seconds: the keys, their handles and the elastic period
    keyframe_points = [
        {
            "amplitude": key.amplitude,
            "back": key.back,
            "easing": key.easing,
            "handle_left": [frames_to_sec(key.handle_left[0], fps), key.handle_left[1]],
            "handle_left_type": key.handle_left_type,
            "handle_right": [frames_to_sec(key.handle_right[0], fps), key.handle_right[1]],
            "handle_right_type": key.handle_right_type,
            "interpolation": key.interpolation,
            "co": [frames_to_sec(key.co[0], fps), key.co[1]],
            "period": frames_to_sec(key.period, fps)
        }
        for key in fcurve.keyframe_points
    ]

    return {
        "array_index": fcurve.array_index,
        "auto_smoothing": fcurve.auto_smoothing,
        "data_path": fcurve.data_path,
        "extrapolation": fcurve.extrapolation,
        "keyframe_points": keyframe_points,
        "range": [frames_to_sec(fcurve.range()[0], fps), frames_to_sec(fcurve.range()[1], fps)]
    }

def execute():
    """The curves of the objects asked for, and the keyframe count of the ones over the limit (those aren't sent).
    The tracker then watches the rest, so their keyframe edits come in with its scene updates."""
    request = json.loads(REQUEST)
    approved = set(request["approved"])
    curves = {}
    large = {}

    for name in request["fetch"]:
        obj = bpy.data.objects.get(name)
        if obj is None:
            continue
        fcurves = FCurvesFromObject(obj) + ShapeKeyFCurvesFromObject(obj)
        count = sum(len(fcurve.keyframe_points) for fcurve in fcurves)
        if count > request["limit"] and name not in approved:
            large[name] = count
            continue
        curves[name] = [get_fcurve_data(fcurve) for fcurve in fcurves]

    # the add-on's tracker, older add-ons don't hand it over
    tracker = globals().get("tracker")
    if tracker is not None:
        tracker.set_watched_objects([name for name in request["watch"] if name not in large])

    return json.dumps({"curves": curves, "large": large})
