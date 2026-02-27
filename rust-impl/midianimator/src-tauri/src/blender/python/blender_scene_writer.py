from typing import Set, List
import bpy
import json
from bpy_extras import anim_utils
from contextlib import suppress

JSON_DATA = r""""""
# JSON_DATA is a static variable that is injected when rust function send_scene_data() is called



def FCurvesFromObject(obj: bpy.types.Object) -> List[bpy.types.FCurve]:
    """Returns FCurves for an object, or [] if none exist."""
    if obj.animation_data is None: return []
    if obj.animation_data.action is None: return []

    if bpy.app.version < (5, 0, 0):
        return list(obj.animation_data.action.fcurves)
    else:
        anim_data = obj.animation_data
        channelbag = anim_utils.action_get_channelbag_for_slot(anim_data.action, anim_data.action_slot)
        return list(channelbag.fcurves) if channelbag else []


def shapeKeyFCurvesFromObject(obj: bpy.types.Object) -> List[bpy.types.FCurve]:
    """Returns shape-key FCurves for an object, or [] if none exist."""
    with suppress(AttributeError):
        if obj.data.shape_keys is None: return []
        if obj.data.shape_keys.animation_data.action is None: return []

        if bpy.app.version < (5, 0, 0):
            return list(obj.data.shape_keys.animation_data.action.fcurves)
        else:
            anim_data = obj.data.shape_keys.animation_data
            channelbag = anim_utils.action_get_channelbag_for_slot(anim_data.action, anim_data.action_slot)
            return list(channelbag.fcurves) if channelbag else []
    return []


def cleanKeyframes(obj: bpy.types.Object, channels: Set = {"all_channels"}):
    """Kept for external use. Not called internally — see clean_all_keyframes."""
    fCurves = FCurvesFromObject(obj)

    if bpy.app.version < (5, 0, 0):
        for fCurve in fCurves:
            if {fCurve.data_path, "all_channels"}.intersection(channels):
                obj.animation_data.action.fcurves.remove(fCurve)
        for fCurve in shapeKeyFCurvesFromObject(obj):
            obj.data.shape_keys.animation_data.action.fcurves.remove(fCurve)
    else:
        anim_data = obj.animation_data
        if anim_data and anim_data.action:
            channelbag = anim_utils.action_get_channelbag_for_slot(anim_data.action, anim_data.action_slot)
            if channelbag:
                for fCurve in fCurves:
                    if {fCurve.data_path, "all_channels"}.intersection(channels):
                        channelbag.fcurves.remove(fCurve)
        if hasattr(obj.data, "shape_keys") and obj.data.shape_keys and obj.data.shape_keys.animation_data:
            sk_anim_data = obj.data.shape_keys.animation_data
            if sk_anim_data.action:
                sk_channelbag = anim_utils.action_get_channelbag_for_slot(sk_anim_data.action, sk_anim_data.action_slot)
                if sk_channelbag:
                    for fCurve in shapeKeyFCurvesFromObject(obj):
                        sk_channelbag.fcurves.remove(fCurve)


def clean_all_keyframes(data: dict):
    """Removes animation data entirely from each object to avoid stale pointer
    crashes on repeated runs. Does not touch frame_current."""
    for object_name in data.keys():
        if object_name.startswith("ANIM"):
            continue

        obj = bpy.data.objects.get(object_name)
        if obj is None:
            print(f"Warning: object '{object_name}' not found in scene, skipping clean.")
            continue

        if obj.animation_data:
            if obj.animation_data.action:
                action = obj.animation_data.action
                obj.animation_data.action = None
                if action.users == 0:
                    bpy.data.actions.remove(action)
            obj.animation_data_clear()


def get_or_create_fcurve(obj: bpy.types.Object, data_path: str, array_index: int) -> bpy.types.FCurve:
    """Gets an existing FCurve or creates a new one for the given data_path/index.
    Always re-fetches obj from bpy.data to avoid stale references."""

    obj = bpy.data.objects[obj.name]

    if obj.animation_data is None:
        obj.animation_data_create()

    if bpy.app.version < (5, 0, 0):
        if obj.animation_data.action is None:
            action = bpy.data.actions.new(name=f"{obj.name}Action")
            obj.animation_data.action = action
        action = obj.animation_data.action
        fc = action.fcurves.find(data_path, index=array_index)
        if fc is None:
            fc = action.fcurves.new(data_path, index=array_index)
    else:
        anim_data = obj.animation_data
        if anim_data.action is None:
            action = bpy.data.actions.new(name=f"{obj.name}Action")
            anim_data.action = action
        action = anim_data.action
        channelbag = anim_utils.action_get_channelbag_for_slot(action, anim_data.action_slot)
        if channelbag is None:
            channelbag = action.channelbags.new(anim_data.action_slot)
        fc = channelbag.fcurves.find(data_path, index=array_index)
        if fc is None:
            fc = channelbag.fcurves.new(data_path, index=array_index)

    return fc


def seconds_to_frame(seconds: float) -> float:
    """Converts a time in seconds to a (possibly fractional) frame number
    using the current scene's FPS and FPS base."""
    scene = bpy.context.scene
    return seconds * (scene.render.fps / scene.render.fps_base)


def write_keyframes(data: dict):
    """Writes keyframes directly to FCurves using FAST insertion to avoid
    depsgraph callbacks on every point. fc.update() is called once per curve."""
    for object_name, keyframe_data in data.items():
        if object_name.startswith("ANIM"):
            continue

        obj = bpy.data.objects.get(object_name)
        if obj is None:
            print(f"Warning: object '{object_name}' not found in scene, skipping.")
            continue

        fcurve_map: dict[tuple, list] = {}
        for keyframe in keyframe_data:
            data_path = keyframe.get("data_path", "")
            if not data_path:
                continue
            key = (data_path, keyframe.get("array_index", 0))
            fcurve_map.setdefault(key, []).append(keyframe)

        for (data_path, array_index), keyframes in fcurve_map.items():
            try:
                fc = get_or_create_fcurve(obj, data_path, array_index)

                frames = [seconds_to_frame(kf["time"]) for kf in keyframes]
                values = [kf["value"] for kf in keyframes]

                fc.keyframe_points.add(len(keyframes))
                fc.keyframe_points.foreach_set("co", [x for co in zip(frames, values) for x in co])
                fc.update()
            except Exception as e:
                print(f"Error writing FCurve '{data_path}[{array_index}]' on '{object_name}': {e}")


def _execute_on_main_thread():
    """Runs on Blender's main thread via app.timers. Must return None to unregister."""
    try:
        data = json.loads(JSON_DATA)
        clean_all_keyframes(data)
        write_keyframes(data)
    except Exception as e:
        print(f"Error in execute: {e}")
    return None 


def execute():
    """Called from the addon thread. Defers all bpy work to the main thread."""
    bpy.app.timers.register(_execute_on_main_thread, first_interval=0.0)
    return "OK"