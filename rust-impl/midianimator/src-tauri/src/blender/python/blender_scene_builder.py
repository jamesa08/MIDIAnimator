import json
import bpy

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
    
    return list(obj.animation_data.action.fcurves)

def get_fcurve_data(fcurve):
    """Converts an FCurve into a dictionary representation.

    :param bpy.types.FCurve fcurve: the Blender FCurve
    :return dict: dictionary representing the FCurve
    """
    keyframe_points = [
        {
            "amplitude": key.amplitude,
            "back": key.back,
            "easing": key.easing, 
            "handle_left": list(key.handle_left),
            "handle_left_type": key.handle_left_type,
            "handle_right": list(key.handle_right),
            "handle_right_type": key.handle_right_type,
            "interpolation": key.interpolation,
            "co": list(key.co),
            "period": key.period
        }
        for key in fcurve.keyframe_points
    ]
    
    return {
        "array_index": fcurve.array_index,
        "auto_smoothing": fcurve.auto_smoothing,
        "data_path": fcurve.data_path,
        "extrapolation": fcurve.extrapolation,
        "keyframe_points": keyframe_points,
        "range": list(fcurve.range()),
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
            fcurves = FCurvesFromObject(obj)
            obj_data["anim_curves"] = [get_fcurve_data(fcurve) for fcurve in fcurves]
        
        objects.append(obj_data)
    
    for child in collection.children:
        objects = get_all_objects_in_collection(child, objects)

    return objects

def execute():
    scene_data = {}

    for scene in bpy.data.scenes:
        scene_key = f"Scene('{scene.name}')"
        scene_data[scene_key] = {}
        scene_data[scene_key]["object_group"] = {}

        for collection in scene.collection.children:
            collection_key = f"ObjectGroup('{collection.name}')"
            scene_data[scene_key]["object_group"][collection_key] = {
                "objects": get_all_objects_in_collection(collection)
            }
    return json.dumps(scene_data)
