# Scene change tracker for MIDIAnimator Bridge
# Detects important scene changes and sends updates via the Server singleton

import bpy
from bpy_extras import anim_utils
from bpy.app.handlers import persistent
import json
import time
import uuid
from . core import Server

# the scene as last sent, compared on every depsgraph update
_last_structure = None
_last_values = None

# Debounce variables
_last_value_change_time = 0
_values_pending = False
_debounce_interval = 0.5  # seconds to wait after the last transform or keyframe change before sending update
_timer_registered = False  # Track if our timer is registered

def generate_uuid():
    """Generate a unique UUID for each message."""
    return str(uuid.uuid4())

def get_structure_signature():
    """The scene layout: every collection's children and objects in order, and each object's name, parent, visibility and actions.
    Any change here is sent right away."""
    collections = []

    def walk(key, collection):
        collections.append((key, tuple(child.name for child in collection.children), tuple(obj.name for obj in collection.objects)))
        for child in collection.children:
            walk(child.name, child)

    for scene in bpy.data.scenes:
        walk(scene.name, scene.collection)

    objects = tuple(
        (obj.name, obj.parent.name if obj.parent else None, obj.visible_get(), action_name(obj), action_name(getattr(obj.data, "shape_keys", None)))
        for obj in bpy.data.objects
    )
    return (tuple(collections), objects)

def action_name(id_data):
    """The name of the action on an object or shape keys, None without one."""
    anim_data = id_data.animation_data if id_data else None
    return anim_data.action.name if anim_data and anim_data.action else None

def get_values_signature():
    """Values that change continuously while dragging: transforms, and the keyframes of ANIM objects.
    Changes are sent once they settle."""
    values = []
    for obj in bpy.data.objects:
        values.append((obj.name, tuple(obj.location), tuple(obj.rotation_euler), tuple(obj.scale)))
        if obj.name.startswith("ANIM"):
            for fcurve in FCurvesFromObject(obj) + ShapeKeyFCurvesFromObject(obj):
                keys = tuple((tuple(key.co), tuple(key.handle_left), tuple(key.handle_right), key.interpolation) for key in fcurve.keyframe_points)
                values.append((obj.name, fcurve.data_path, fcurve.array_index, keys))
    return tuple(values)

def shape_keys_from_object(obj):
    """gets shape keys from object"""
    if obj is None: return [], None
    if obj.type not in ("MESH", "CURVE", "LATTICE"): return [], None
    if obj.data.shape_keys is None: return [], None

    reference = obj.data.shape_keys.reference_key
    return list(obj.data.shape_keys.key_blocks)[1:], reference

def frames_to_sec(frames, fps):
    """Converts frames to seconds based on the given frames per second."""
    return frames / fps

def FCurvesFromObject(obj):
    """Gets FCurves from an object."""
    if obj.animation_data is None: return []
    if obj.animation_data.action is None: return []
    
    if bpy.app.version < (5, 0, 0):
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

    if bpy.app.version < (5, 0, 0):
        return list(shape_keys.animation_data.action.fcurves)
    else:
        anim_data = shape_keys.animation_data
        channelbag = anim_utils.action_get_channelbag_for_slot(anim_data.action, anim_data.action_slot)
        return list(channelbag.fcurves) if channelbag else []

def get_fcurve_data(fcurve):
    """Converts an FCurve into a dictionary representation."""
    fps = bpy.context.scene.render.fps
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
            "co": [frames_to_sec(key.co[0], fps), key.co[1]],
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
        "range": [frames_to_sec(fcurve.range()[0], fps), frames_to_sec(fcurve.range()[1], fps)],
    }

def get_all_objects_in_collection(collection, objects=None):
    if objects is None:
        objects = []
    
    for obj in collection.objects:
        keys, ref = shape_keys_from_object(obj)
        obj_data = {
            "name": obj.name,
            "position": list(obj.location),
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
    """Generates a JSON representation of the current scene data matching Rust structs."""
    scene_data = {}

    for scene in bpy.data.scenes:
        scene_key = f"{scene.name}"
        object_groups = []

        for collection in scene.collection.children:
            # Prepare objects, changing "location" to "position"
            objects = []
            for obj in collection.objects:
                keys, ref = shape_keys_from_object(obj)
                obj_data = {
                    "name": obj.name,
                    "position": list(obj.location),  # Changed from "location" to "position"
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
            
            # Add objects from child collections
            for child in collection.children:
                objects = get_all_objects_in_collection(child, objects)
            
            object_group = {
                "name": collection.name,
                "objects": objects
            }
            object_groups.append(object_group)
            
        scene_data[scene_key] = {
            "name": scene.name,
            "object_groups": object_groups
        }
            
    return json.dumps(scene_data)

def send_scene_update(scene_data, change_type=None, changed_data=None):
    """Send scene data through the socket server."""
    server = Server()
    if server.connected:
        try:
            # Generate a unique UUID for each message
            message_uuid = generate_uuid()
            print(f"Sending scene update with UUID: {message_uuid}")
            server.send_message(scene_data, message_uuid)
        except Exception as e:
            print(f"Failed to send scene update: {e}")

def check_pending_transforms():
    """Send the scene once transforms and keyframes stopped changing for the debounce interval."""
    global _values_pending

    if _values_pending and (time.time() - _last_value_change_time) > _debounce_interval:
        _values_pending = False
        print("Sending pending transform changes")
        send_scene_update(execute(), "transform_change")
        return True
    return False

@persistent
def detect_important_changes(scene, depsgraph):
    """Send the scene when anything in it changed: layout changes right away, transforms and keyframes once they settle."""
    global _last_structure, _last_values, _last_value_change_time, _values_pending

    if not depsgraph:
        return

    structure = get_structure_signature()
    values = get_values_signature()

    if structure != _last_structure:
        _last_structure = structure
        _last_values = values
        # the full scene goes out now, it includes any pending transforms
        _values_pending = False
        print("Important change detected: scene_change")
        send_scene_update(execute(), "scene_change")
    elif values != _last_values:
        # debounced, the timer sends it
        _last_values = values
        _last_value_change_time = time.time()
        _values_pending = True

# Add a timer function to check for pending transforms
def check_transforms_timer():
    check_pending_transforms()
    return 0.1  # Check every 0.1 seconds

def initialize_trackers():
    """Record the current scene and send it."""
    global _last_structure, _last_values, _values_pending

    _last_structure = get_structure_signature()
    _last_values = get_values_signature()
    _values_pending = False

    # Send initial state
    send_scene_update(execute(), "initial_state", None)

def register_tracker():
    """Register the scene change tracker."""
    global _timer_registered
    
    # Remove any existing handlers first
    unregister_tracker()
    
    # Initialize trackers
    initialize_trackers()
    
    # Register to update events
    bpy.app.handlers.depsgraph_update_post.append(detect_important_changes)
    
    # Add timer for transform checks
    try:
        # Safe timer registration
        bpy.app.timers.register(check_transforms_timer, persistent=True)
        _timer_registered = True
    except Exception as e:
        print(f"Error registering timer: {e}")
    
    print("Scene tracker registered")

def unregister_tracker():
    """Unregister the scene change tracker."""
    global _timer_registered
    
    # Remove depsgraph handler
    if detect_important_changes in bpy.app.handlers.depsgraph_update_post:
        bpy.app.handlers.depsgraph_update_post.remove(detect_important_changes)
    
    # Remove timer
    try:
        # Safe timer unregistration - directly unregister by function reference
        if _timer_registered:
            bpy.app.timers.unregister(check_transforms_timer)
        _timer_registered = False
    except Exception as e:
        # Timer might not be registered, which is fine
        _timer_registered = False
    
    print("Scene tracker unregistered")

# Operator classes
class SCENE_OT_StartSceneTracker(bpy.types.Operator):
    bl_idname = "scene.start_tracker"
    bl_label = "Start Scene Tracker"
    bl_description = "Start tracking scene changes"
    
    def execute(self, context):
        # Ensure server is connected first
        server = Server()
        if not server.connected:
            self.report({'ERROR'}, "Not connected to server")
            return {'CANCELLED'}
        
        register_tracker()
        self.report({'INFO'}, "Scene tracker started")
        return {'FINISHED'}

class SCENE_OT_StopSceneTracker(bpy.types.Operator):
    bl_idname = "scene.stop_tracker"
    bl_label = "Stop Scene Tracker"
    bl_description = "Stop tracking scene changes"
    
    def execute(self, context):
        unregister_tracker()
        self.report({'INFO'}, "Scene tracker stopped")
        return {'FINISHED'}