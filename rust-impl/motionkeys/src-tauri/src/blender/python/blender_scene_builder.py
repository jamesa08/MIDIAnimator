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

def scene_fps(scene):
    """The scene's frame rate with its base (29.97 is 30 / 1.001), the same one the scene writer converts with."""
    return scene.render.fps / scene.render.fps_base

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
            # MotionKeys fetches the curves of the objects its graph reads on its own (blender_object_curves.py)
            "anim_curves": [],
        }

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
