# Scene change tracker for MotionKeys Bridge
# Detects important scene changes and sends updates via the Server singleton: the scene when its layout changes, the
# curves of watched objects when their keys change

import bpy
from bpy.app.handlers import persistent
from mathutils import Quaternion
import math
import time
from . core import Server
from .animation import fcurves_of, get_enums, object_channels, object_curves, object_fcurves, scene_fps, shape_keys_of

# the scene's layout as last sent, compared on every depsgraph update
_last_structure = None

# the objects MotionKeys reads curves from (set_watched_objects)
_watched = set()
# what MotionKeys has of each watched object: {name: (signature, curves)}, curves None when it got them some other
# way. a curves event sends the objects whose curves differ from these
_sent = {}
# the watched objects' signatures at the last depsgraph update, a change restarts the debounce
_seen = {}

# Debounce variables
_last_curves_change_time = 0
_curves_pending = False
_debounce_interval = 0.5  # seconds to wait after the last keyframe change before sending the curves
_timer_registered = False  # Track if our timer is registered

def get_structure_signature():
    """The layout of the scene the user is working in: its name and frame rate, every collection's children and
    objects in order, and what each object's channels are listed from (object_layout). Any change here is sent right
    away, switching scenes too. Transforms aren't in it, moving an object never sends the scene. It runs on every
    depsgraph update, so it only reads attributes, no data paths are resolved."""
    scene = bpy.context.scene
    collections = []

    def walk(key, collection):
        collections.append((key, tuple(child.name for child in collection.children), tuple(obj.name for obj in collection.objects)))
        for child in collection.children:
            walk(child.name, child)

    walk(scene.name, scene.collection)

    objects = tuple(object_layout(obj) for obj in scene.objects)
    return (scene.name, scene_fps(scene), tuple(collections), objects)

def object_layout(obj):
    """An object's name, parent and visibility, and everything its channel list comes from: its actions, their
    F-curves and whether each has keys, its shape keys' names, its bones' names, the rotation modes of the object and
    its bones, and once it's animated the names of its and its bones' custom properties."""
    shape_keys = shape_keys_of(obj)
    fcurves = fcurves_layout(obj)
    # a custom property's F-curve is listed under the property's key, once the property is renamed it's raw
    animated = bool(fcurves)
    return (
        obj.name,
        obj.parent.name if obj.parent else None,
        obj.visible_get(),
        obj.rotation_mode,
        action_name(obj),
        fcurves,
        tuple(obj.keys()) if animated else None,
        action_name(shape_keys),
        fcurves_layout(shape_keys),
        tuple(block.name for block in shape_keys.key_blocks) if shape_keys else None,
        tuple((bone.name, bone.rotation_mode, tuple(bone.keys()) if animated else None) for bone in obj.pose.bones) if obj.pose else None,
    )

def fcurves_layout(id_data):
    """The data path and index of each F-curve of an object's or shape keys' action, and whether it has keys."""
    return tuple((fcurve.data_path, fcurve.array_index, len(fcurve.keyframe_points) > 0) for fcurve in fcurves_of(id_data))

def action_name(id_data):
    """The name of the action on an object or shape keys, None without one."""
    anim_data = id_data.animation_data if id_data else None
    return anim_data.action.name if anim_data and anim_data.action else None

def watched_signatures():
    """The keys of the watched objects in the scene, by object name. They change continuously while dragging, so
    changes are sent once they settle. Transforms aren't watched: they go out with the scene, but moving an object
    doesn't send it."""
    scene = bpy.context.scene
    fps = scene_fps(scene)
    signatures = {}
    for name in _watched:
        obj = scene.objects.get(name)
        if obj is not None:
            signatures[name] = object_signature(obj, fps)
    return signatures

def object_signature(obj, fps):
    """Everything an object's curves are made from: its F-curves' keys and modifiers, and the frame rate that turns
    frames into seconds."""
    return (fps, tuple((fcurve.data_path, fcurve.array_index, keyframes_signature(fcurve), modifiers_signature(fcurve)) for fcurve in object_fcurves(obj)))

def keyframes_signature(fcurve):
    """An F-curve's keys, handles, interpolations and easings as flat tuples. foreach_get reads them in one go, an
    object with thousands of keys is compared on every depsgraph update"""
    points = fcurve.keyframe_points
    signature = []
    for prop, size in (("co", 2), ("handle_left", 2), ("handle_right", 2), ("back", 1), ("amplitude", 1), ("period", 1)):
        values = [0] * (len(points) * size)
        points.foreach_get(prop, values)
        signature.append(tuple(values))
    for prop in ("interpolation", "easing"):
        signature.append(tuple(get_enums(points, prop)))
    return tuple(signature)

# modifier settings that only change how it's shown
UI_PROPERTIES = {"rna_type", "name", "show_expanded", "active", "is_valid"}

def rna_signature(struct):
    """The settings of a modifier (or one of its control points) as a tuple."""
    values = []
    for prop in struct.bl_rna.properties:
        if prop.identifier in UI_PROPERTIES or prop.type == "POINTER":
            continue
        value = getattr(struct, prop.identifier)
        if prop.type == "COLLECTION":
            value = tuple(rna_signature(item) for item in value)
        elif isinstance(value, set):
            value = tuple(sorted(value))
        elif not isinstance(value, (bool, int, float, str)):
            value = tuple(value)
        values.append(value)
    return tuple(values)

def modifiers_signature(fcurve):
    """An F-curve's modifiers and their settings, a Noise modifier's strength changes the curve too."""
    return tuple(rna_signature(modifier) for modifier in fcurve.modifiers)

def set_watched_objects(names, sent):
    """Sets the objects MotionKeys reads curves from, their curves go out when their keys change. `sent` holds the
    curves MotionKeys was just sent. The others it already has: their changes still to send stay pending."""
    global _watched, _sent, _seen
    _watched = set(names)
    scene = bpy.context.scene
    fps = scene_fps(scene)
    previous = _sent
    _sent = {}
    for name in _watched:
        obj = scene.objects.get(name)
        if name in sent:
            _sent[name] = (object_signature(obj, fps), sent[name])
        elif name in previous:
            _sent[name] = previous[name]
        elif obj is not None:
            _sent[name] = (object_signature(obj, fps), None)
    _seen = watched_signatures()

# the group of the objects linked straight to the scene collection, outside every collection. collections' ids start
# with "collection:", so a collection named like it never takes its place
SCENE_GROUP = "Scene group"
SCENE_GROUP_ID = "scene"

def collection_group_id(collection):
    """A collection's group id for MotionKeys."""
    return f"collection:{collection.name}"

# Blender's axes: z up, right-handed, rotations in radians
AXES = {"up": "z", "handedness": "right", "angle_unit": "radians"}

# the rotation modes that are Euler angles, named by their order
EULER_ORDERS = {"XYZ", "XZY", "YXZ", "YZX", "ZXY", "ZYX"}

def rotation_order(obj):
    """The Euler order of an object's rotation, XYZ for one in quaternion or axis-angle mode."""
    return obj.rotation_mode if obj.rotation_mode in EULER_ORDERS else "XYZ"

def euler_rotation(obj):
    """An object's rotation as Euler angles in its rotation order, converted to XYZ from a quaternion or axis-angle."""
    if obj.rotation_mode == "QUATERNION":
        return obj.rotation_quaternion.to_euler("XYZ")
    if obj.rotation_mode == "AXIS_ANGLE":
        angle, x, y, z = obj.rotation_axis_angle
        return Quaternion((x, y, z), angle).to_euler("XYZ")
    return obj.rotation_euler

def finite(values):
    """A vector as a list, a number that isn't finite is 0 so the scene stays valid JSON."""
    return [value if math.isfinite(value) else 0.0 for value in values]

def get_object_data(obj):
    """An object as MotionKeys' Scene has it: its transform for display and the channels it can be written to. Its
    curves go out on their own (handlers.watch, send_curves_update)."""
    return {
        # the object's id for MotionKeys, object names are unique in a file
        "id": obj.name,
        "name": obj.name,
        "rotation_order": rotation_order(obj),
        "transform": {"position": finite(obj.location), "rotation": finite(euler_rotation(obj)), "scale": finite(obj.scale)},
        "channels": object_channels(obj),
    }

def get_all_objects_in_collection(collection, built, objects=None):
    """The objects of a collection and its nested collections. `built` holds the objects already built, an object in
    several groups is built once."""
    if objects is None:
        objects = []

    for obj in collection.objects:
        objects.append(built_object(obj, built))

    for child in collection.children:
        get_all_objects_in_collection(child, built, objects)

    return objects

def built_object(obj, built):
    """An object's data, built the first time it's asked for."""
    if obj.name not in built:
        built[obj.name] = get_object_data(obj)
    return built[obj.name]

def build_scene():
    """The scene the user is working in, in the shape of MotionKeys' Scene. Each top-level collection is a group
    with its nested collections' objects in it, the objects outside every collection come last in their own group."""
    scene = bpy.context.scene
    groups = []
    built = {}

    for collection in scene.collection.children:
        groups.append({"id": collection_group_id(collection), "name": collection.name, "objects": get_all_objects_in_collection(collection, built)})

    if scene.collection.objects:
        groups.append({"id": SCENE_GROUP_ID, "name": SCENE_GROUP, "objects": [built_object(obj, built) for obj in scene.collection.objects]})

    return {"id": scene.name, "name": scene.name, "fps": scene_fps(scene), "axes": AXES, "object_groups": groups}

def send_scene_update(change_type):
    """Sends the scene to MotionKeys, unasked."""
    server = Server()
    if server.connected:
        try:
            print(f"Sending scene update ({change_type})")
            server.send("scene", build_scene())
        except Exception as e:
            print(f"Failed to send scene update: {e}")

def send_curves_update():
    """Sends a curves event with every watched object whose curves changed since MotionKeys last got them."""
    scene = bpy.context.scene
    fps = scene_fps(scene)
    updates = {}
    changed = {}
    for name in _watched:
        obj = scene.objects.get(name)
        if obj is None:
            continue
        signature = object_signature(obj, fps)
        sent_signature, sent_curves = _sent.get(name, (None, None))
        if signature == sent_signature:
            continue
        curves = object_curves(obj, fps)
        updates[name] = (signature, curves)
        # keys moved and moved back, or a change that leaves the curves as they were, isn't sent
        if curves != sent_curves:
            changed[name] = curves

    server = Server()
    if changed:
        if not server.connected:
            return
        try:
            print(f"Sending curves update ({len(changed)} objects)")
            server.send("curves", {"objects": changed})
        except Exception as e:
            # MotionKeys didn't get them, they're still changes
            print(f"Failed to send curves update: {e}")
            return
    _sent.update(updates)

def check_pending_curves():
    """Send the changed curves once keys stopped changing for the debounce interval."""
    global _curves_pending

    if _curves_pending and (time.time() - _last_curves_change_time) > _debounce_interval:
        _curves_pending = False
        send_curves_update()
        return True
    return False

@persistent
def detect_important_changes(scene, depsgraph):
    """Send the scene right away when its layout changed, and the curves of watched objects once their keys settle."""
    global _last_structure, _seen, _last_curves_change_time, _curves_pending

    if not depsgraph:
        return

    structure = get_structure_signature()
    if structure != _last_structure:
        _last_structure = structure
        print("Important change detected: scene_change")
        send_scene_update("scene_change")

    signatures = watched_signatures()
    if signatures != _seen:
        # debounced, the timer sends it
        _seen = signatures
        _last_curves_change_time = time.time()
        _curves_pending = True

# Add a timer function to check for pending curves
def check_curves_timer():
    check_pending_curves()
    return 0.1  # Check every 0.1 seconds

def initialize_trackers():
    """Record the current scene and send it."""
    global _last_structure, _seen, _curves_pending

    _last_structure = get_structure_signature()
    _seen = watched_signatures()
    _curves_pending = False

    # Send initial state
    send_scene_update("initial_state")

def register_tracker():
    """Register the scene change tracker."""
    global _timer_registered
    
    # Remove any existing handlers first
    unregister_tracker()
    
    # Initialize trackers
    initialize_trackers()
    
    # Register to update events
    bpy.app.handlers.depsgraph_update_post.append(detect_important_changes)
    
    # Add timer for curve checks
    try:
        # Safe timer registration
        bpy.app.timers.register(check_curves_timer, persistent=True)
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
            bpy.app.timers.unregister(check_curves_timer)
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