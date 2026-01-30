# Scene change tracker for MIDIAnimator Bridge
# Detects important scene changes and sends updates via the Server singleton

import bpy
from bpy_extras import anim_utils
from bpy.app.handlers import persistent
import json
import time
import uuid
from . core import Server

# Track object names and states to detect specific changes
_tracked_objects = {}
_tracked_collections = set()

# Debounce variables
_last_transform_time = 0
_pending_transform_changes = {}
_debounce_interval = 0.5  # seconds to wait after last transform before sending update
_timer_registered = False  # Track if our timer is registered

def generate_uuid():
    """Generate a unique UUID for each message."""
    return str(uuid.uuid4())

def get_object_signature(obj):
    """Create a lightweight signature of an object's critical properties."""
    return {
        "name": obj.name,
        "location": list(obj.location),
        "rotation": list(obj.rotation_euler),
        "scale": list(obj.scale),
        "visible": obj.visible_get(),
        "parent": obj.parent.name if obj.parent else None,
        "animation": bool(obj.animation_data and obj.animation_data.action)
    }

def get_collection_signature(collection):
    """Create a signature for a collection."""
    return {
        "name": collection.name,
        "objects": set(obj.name for obj in collection.objects),
        "visible": not collection.hide_viewport
    }

def shape_keys_from_object(obj):
    """gets shape keys from object"""
    if obj is None: return [], None
    if obj.type not in ("MESH", "CURVE", "LATTICE"): return [], None
    if obj.data.shape_keys is None: return [], None

    reference = obj.data.shape_keys.reference_key
    return list(obj.data.shape_keys.key_blocks)[1:], reference

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

def get_fcurve_data(fcurve):
    """Converts an FCurve into a dictionary representation."""
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
                    fcurves = FCurvesFromObject(obj)
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
    """Check if there are pending transform changes that should be sent."""
    global _last_transform_time, _pending_transform_changes
    
    current_time = time.time()
    if _pending_transform_changes and (current_time - _last_transform_time) > _debounce_interval:
        print("Sending pending transform changes")
        send_scene_update(execute(), "transform_change", {"objects": list(_pending_transform_changes.keys())})
        _pending_transform_changes.clear()
        return True
    return False

@persistent
def detect_important_changes(scene, depsgraph):
    """Focus on detecting important changes only."""
    global _last_transform_time, _pending_transform_changes
    
    if not depsgraph:
        return
    
    # First check if we should send any pending transform changes
    if check_pending_transforms():
        return
    
    important_change = False
    change_type = None
    changed_data = {}
    is_transform_change = False
    
    # Check for specific updates in the depsgraph
    for update in depsgraph.updates:
        # New or updated objects
        if update.id and isinstance(update.id, bpy.types.Object):
            obj = update.id
            
            # New object - always send immediately
            if obj.name not in _tracked_objects:
                important_change = True
                change_type = "new_object"
                changed_data = {"object": obj.name}
                _tracked_objects[obj.name] = get_object_signature(obj)
                break
                
            # Check for important property changes
            old_sig = _tracked_objects[obj.name]
            new_sig = get_object_signature(obj)
            
            if old_sig["name"] != new_sig["name"]:
                # Name changes are sent immediately
                important_change = True
                change_type = "rename_object"
                changed_data = {"old_name": old_sig["name"], "new_name": new_sig["name"]}
                
            elif old_sig["visible"] != new_sig["visible"]:
                # Visibility changes are sent immediately
                important_change = True
                change_type = "visibility_change"
                changed_data = {"object": obj.name, "visible": new_sig["visible"]}
                
            elif old_sig["parent"] != new_sig["parent"]:
                # Parent changes are sent immediately
                important_change = True
                change_type = "parent_change"
                changed_data = {"object": obj.name, "parent": new_sig["parent"]}
                
            elif old_sig["animation"] != new_sig["animation"]:
                # Animation changes are sent immediately
                important_change = True
                change_type = "animation_change"
                changed_data = {"object": obj.name}
                
            # Check for transform changes - these are debounced
            elif (
                old_sig["location"] != new_sig["location"] or
                old_sig["rotation"] != new_sig["rotation"] or
                old_sig["scale"] != new_sig["scale"]
            ):
                # For transform changes, we just record the time and object
                _last_transform_time = time.time()
                _pending_transform_changes[obj.name] = True
                is_transform_change = True
            
            # Update the tracked signature
            _tracked_objects[obj.name] = new_sig
        
        # Collection changes - always send immediately
        elif update.id and isinstance(update.id, bpy.types.Collection):
            collection = update.id
            
            # New collection
            if collection.name not in _tracked_collections:
                important_change = True
                change_type = "new_collection"
                changed_data = {"collection": collection.name}
                _tracked_collections.add(collection.name)
                break
            
            # Get current objects in collection
            current_objects = set(obj.name for obj in collection.objects)
            old_objects = set()
            
            # Check for added or removed objects in collection
            if collection.name in _tracked_collections:
                # Create a temporary signature just for comparison
                old_sig = get_collection_signature(collection)
                old_objects = old_sig["objects"]
                
                added_objects = current_objects - old_objects
                removed_objects = old_objects - current_objects
                
                if added_objects or removed_objects:
                    important_change = True
                    change_type = "collection_membership"
                    changed_data = {
                        "collection": collection.name,
                        "added": list(added_objects),
                        "removed": list(removed_objects)
                    }
    
    # Check for deleted objects - always send immediately
    current_objects = {obj.name for obj in bpy.data.objects}
    deleted_objects = set(_tracked_objects.keys()) - current_objects
    if deleted_objects:
        important_change = True
        change_type = "deleted_objects"
        changed_data = {"objects": list(deleted_objects)}
        for obj_name in deleted_objects:
            if obj_name in _tracked_objects:
                del _tracked_objects[obj_name]
    
    # Check for deleted collections - always send immediately
    current_collections = {col.name for col in bpy.data.collections}
    deleted_collections = set()
    for col_name in _tracked_collections:
        if col_name not in current_collections:
            deleted_collections.add(col_name)
    
    if deleted_collections:
        important_change = True
        change_type = "deleted_collections"
        changed_data = {"collections": list(deleted_collections)}
        for col_name in deleted_collections:
            _tracked_collections.remove(col_name)
    
    # If important non-transform change detected, send the full scene data immediately
    if important_change and not is_transform_change:
        print(f"Important change detected: {change_type}")
        print(f"Changed data: {changed_data}")
        send_scene_update(execute(), change_type, changed_data)

# Add a timer function to check for pending transforms
def check_transforms_timer():
    check_pending_transforms()
    return 0.1  # Check every 0.1 seconds

def initialize_trackers():
    """Initialize trackers for all existing objects and collections."""
    _tracked_objects.clear()
    _tracked_collections.clear()
    _pending_transform_changes.clear()
    
    # Track all existing objects
    for obj in bpy.data.objects:
        _tracked_objects[obj.name] = get_object_signature(obj)
    
    # Track all existing collections
    for collection in bpy.data.collections:
        _tracked_collections.add(collection.name)
        
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