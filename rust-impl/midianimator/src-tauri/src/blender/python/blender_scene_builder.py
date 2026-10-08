import json
import bpy
from bpy_extras import anim_utils

# action slots came in Blender 4.4. from then on the old action.fcurves API only reaches a legacy slot the object isn't
# assigned to, so keys written through it never animate, and reading it can miss the slot the object uses
SLOTTED_ACTIONS = bpy.app.version >= (4, 4, 0)

def shape_keys_from_object(obj):
    """gets shape keys from object

    :param bpy.types.Object object: the blender object
    :return tuple: returns a tuple, first element being a list of the shape keys and second element being the reference key
    """
    # from Animation Nodes
    # https://github.com/JacquesLucke/animation_nodes/blob/7a74e31fca0e7fce6edefdb8183dc4ac9c5acbfc/animation_nodes/nodes/shape_key/shape_keys_from_object.py
    
    if obj is None: return [], None
    if obj.type not in ("MESH", "CURVE", "LATTICE"): return [], None
    if obj.data.shape_keys is None: return [], None

    reference = obj.data.shape_keys.reference_key
    return list(obj.data.shape_keys.key_blocks)[1:], reference

def FCurvesFromObject(obj):
    """Gets FCurves (`bpy.types.FCurve`) from an object (`bpy.types.Object`).

    :param bpy.types.Object obj: the Blender object
    :return List[bpy.types.FCurve]: a list of `bpy.types.FCurve` objects. If the object does not have FCurves, it will return an empty list.
    """
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
    """Converts an FCurve into a dictionary representation.

    :param bpy.types.FCurve fcurve: the Blender FCurve
    :return dict: dictionary representing the FCurve
    """
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

def get_all_objects_in_collection(collection, objects=None):
    if objects is None:
        objects = []
    
    for obj in collection.objects:
        keys, ref = shape_keys_from_object(obj)
        obj_data = {
            "name": obj.name,
            "location": list(obj.location),
            "rotation": list(obj.rotation_euler),
            "scale": list(obj.scale),
            "blend_shapes": {
                "keys": [repr(key) for key in keys if key is not None],
                "reference": repr(ref) if ref is not None else None
            },
            "anim_curves": [],
        }
        
        if obj.name.startswith("ANIM"):
            fcurves = FCurvesFromObject(obj) + ShapeKeyFCurvesFromObject(obj)
            obj_data["anim_curves"] = [get_fcurve_data(fcurve) for fcurve in fcurves]
        
        objects.append(obj_data)
    
    for child in collection.children:
        objects = get_all_objects_in_collection(child, objects)

    return objects

def execute():
    scene_data = {}

    for scene in bpy.data.scenes:
        scene_key = f"{scene.name}"
        scene_data[scene_key] = {}
        scene_data[scene_key]["object_group"] = {}
        scene_data[scene_key]["fps"] = scene_fps(scene)

        for collection in scene.collection.children:
            collection_key = f"{collection.name}"
            scene_data[scene_key]["object_group"][collection_key] = {
                "objects": get_all_objects_in_collection(collection)
            }
    return json.dumps(scene_data)
