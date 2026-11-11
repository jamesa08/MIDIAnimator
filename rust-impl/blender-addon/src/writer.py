# writes the keys MotionKeys sends, as given: times, values, interpolations and handles (docs/bridge_protocol.md §7.5,
# §8.8). runs on Blender's main thread (core.Server answers requests there)

import math
import re
from bisect import bisect_left
from contextlib import suppress

import bpy

from .animation import KEYFRAME_ENUMS, SLOTTED_ACTIONS, f32, fcurves_of, get_enums, is_array, is_scalar, resolve, scene_fps, set_enums, shape_keys_of


def clean_all_keyframes(object_names):
    """Removes animation data entirely from each object to avoid stale pointer crashes on repeated runs. Does not touch
    frame_current."""
    for object_name in object_names:
        obj = bpy.data.objects.get(object_name)
        if obj is None:
            continue

        # the object's own animation and its shape keys'
        for owner in (obj, getattr(obj.data, "shape_keys", None)):
            if owner is None or owner.animation_data is None:
                continue
            if owner.animation_data.action:
                action = owner.animation_data.action
                owner.animation_data.action = None
                if action.users == 0:
                    bpy.data.actions.remove(action)
            owner.animation_data_clear()


class CurveError(Exception):
    """A curve that can't be written, the write reports it and goes on with the others."""


# a channel id's index at its end: location[2], ["color"][1]
INDEX = re.compile(r"^(.+)\[(\d+)\]$")

# a scalar custom property channel: the object's ["wear"], or a bone's pose.bones["Arm"]["wear"]. the name has its \ and
# " escaped (docs/bridge_protocol.md §8.5), the same way Blender escapes them in its paths
CUSTOM_PROPERTY = re.compile(r'^(.*)\["((?:[^"\\]|\\.)+)"\]$')
ESCAPED = re.compile(r"\\(.)")


def ensure_custom_property(owner, data_path):
    """Creates a missing scalar custom property for `data_path`, set to 0.0, so a map (a wear value, say) can be written
    to an object that doesn't have the property yet, the way the writer already makes the action and F-curve it needs.
    It sits on `owner` for a bare ["wear"], or on the bone or struct the prefix names (pose.bones["Arm"]["wear"]).
    Returns (owner, data_path, 0) once it exists, or None when `data_path` isn't a custom property we can make here."""
    match = CUSTOM_PROPERTY.match(data_path)
    if not match:
        return None
    prefix, key = match.group(1), ESCAPED.sub(r"\1", match.group(2))
    holder = owner
    if prefix:
        try:
            holder = resolve(owner, prefix)
        except (ValueError, TypeError, AttributeError):
            return None
    # only make it on something that takes custom properties, reached only when it doesn't already resolve
    try:
        holder[key] = 0.0
    except (TypeError, KeyError):
        return None
    return owner, data_path, 0


def resolve_channel(obj, channel):
    """The datablock, data path and array index a channel id writes to. Shape keys (`key_blocks["Smile"].value`) are
    on the object's shape keys, everything else is on the object. `data_path[0]` on a single value is taken too."""
    owner, where = obj, f"'{obj.name}'"
    if channel.startswith("key_blocks["):
        owner, where = shape_keys_of(obj), f"the shape keys of '{obj.name}'"
        if owner is None:
            raise CurveError(f"'{obj.name}' has no shape keys")

    def value_at(data_path):
        try:
            return True, resolve(owner, data_path)
        except (ValueError, TypeError, AttributeError):
            return False, None

    match = INDEX.match(channel)
    if match:
        data_path, index = match.group(1), int(match.group(2))
        found, value = value_at(data_path)
        if found and is_array(value):
            if index >= len(value):
                raise CurveError(f"'{data_path}' on {where} has {len(value)} values, there's no [{index}]")
            return owner, data_path, index
        if found and is_scalar(value) and index == 0:
            return owner, data_path, 0
        # a scalar custom property written as ["wear"][0], made if it's missing
        if not found and index == 0:
            made = ensure_custom_property(owner, data_path)
            if made is not None:
                return made
    found, value = value_at(channel)
    if not found:
        # a custom property the object doesn't have yet (a wear map's target), made so the write can land
        made = ensure_custom_property(owner, channel)
        if made is not None:
            return made
        raise CurveError(f"{where} has no property '{channel}'")
    if is_array(value):
        raise CurveError(f"'{channel}' on {where} has {len(value)} values, the channel is one of {channel}[0] to {channel}[{len(value) - 1}]")
    if not is_scalar(value):
        raise CurveError(f"'{channel}' on {where} isn't a value that can be animated")
    return owner, channel, 0


def find_fcurve(owner, data_path, array_index):
    """The F-curve animating `data_path[array_index]` on a datablock, None without one."""
    return next((fcurve for fcurve in fcurves_of(owner) if fcurve.data_path == data_path and fcurve.array_index == array_index), None)


def get_or_create_fcurve(obj, owner, data_path, array_index):
    """Gets an existing FCurve or creates a new one for the given data_path/index on `owner`, the object or its shape
    keys. Returns the F-curve and whether it's new."""
    fc = find_fcurve(owner, data_path, array_index)
    if fc is not None:
        return fc, False

    action_name = f"{obj.name}ShapeKeysAction" if owner != obj else f"{obj.name}Action"
    if owner.animation_data is None:
        owner.animation_data_create()

    if not SLOTTED_ACTIONS:
        if owner.animation_data.action is None:
            action = bpy.data.actions.new(name=action_name)
            owner.animation_data.action = action
        action = owner.animation_data.action
        fc = action.fcurves.find(data_path, index=array_index)
        if fc is None:
            fc = action.fcurves.new(data_path, index=array_index)
    else:
        anim_data = owner.animation_data
        if anim_data.action is None:
            action = bpy.data.actions.new(name=action_name)
            anim_data.action = action
        # creates the slot, layer, strip and channelbag if missing
        fc = anim_data.action.fcurve_ensure_for_datablock(owner, data_path, index=array_index)
        # keys in a slot the object doesn't use never play, that has to be an error instead of a silent success
        if anim_data.action_slot is None:
            raise RuntimeError(f"'{owner.name}' has no action slot assigned, its keys wouldn't play")

    return fc, True


def is_number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def parse_keys(keys):
    """A curve's keys checked against §8.6: [(time, value, interpolation, left handle, right handle)], handles as
    (time, value), at the key itself for a key without them. Raises CurveError for an invalid curve."""
    if not isinstance(keys, list):
        raise CurveError("'keys' must be an array of keys")
    parsed = []
    previous = None
    for i, key in enumerate(keys):
        if not isinstance(key, list) or len(key) not in (3, 7):
            raise CurveError(f"key {i} isn't an array of 3 or 7 elements")
        if not all(is_number(x) for x in key[:2] + key[3:]):
            raise CurveError(f"key {i} has a number that isn't finite, or something that isn't a number")
        time, value, interpolation = key[0], key[1], key[2]
        if interpolation not in ("bezier", "linear", "constant"):
            raise CurveError(f"key {i} has the interpolation {interpolation!r}, not \"bezier\", \"linear\" or \"constant\"")
        if previous is not None and time <= previous[0]:
            raise CurveError(f"key {i} isn't after key {i - 1}, keys must have increasing times")
        if len(key) == 3 and (interpolation == "bezier" or (previous is not None and previous[2] == "bezier")):
            raise CurveError(f"key {i} needs its handles, it or the key before it is Bezier")
        if len(key) == 7:
            left, right = (key[3], key[4]), (key[5], key[6])
        else:
            # no handles: both at the key, so nothing reaches out from it
            left = right = (time, value)
        previous = (time, value, interpolation, left, right)
        parsed.append(previous)
    return parsed


# keys within this many frames of a written key are the same key, the written one replaces them
SAME_FRAME = 0.01

INTERPOLATION_VALUES = KEYFRAME_ENUMS["interpolation"]
FREE_HANDLE = KEYFRAME_ENUMS["handle_left_type"]["FREE"]
BLENDER_INTERPOLATIONS = {"bezier": "BEZIER", "linear": "LINEAR", "constant": "CONSTANT"}
# the keyframe enums a write sets
ENUMS = ("interpolation", "handle_left_type", "handle_right_type")


def write_curve(fc, keys, fps):
    """Writes parsed keys to an F-curve as given, with FREE handles so Blender keeps their positions. A key already
    within SAME_FRAME of a written one is replaced, the others stay. Returns how many keys were written."""
    if not keys:
        return 0
    frames = [key[0] * fps for key in keys]
    # a handle on the wrong side of its key goes to the key's frame (§8.7 step 1), Blender's handle recalculation
    # does that too, and its evaluation then matches MotionKeys'
    handles = [(min(left[0] * fps, frame), left[1], max(right[0] * fps, frame), right[1]) for frame, (time, value, interpolation, left, right) in zip(frames, keys)]

    # the keys already there that a written key replaces, last first so the indices stay right
    points = fc.keyframe_points
    existing = [0.0] * (len(points) * 2)
    points.foreach_get("co", existing)
    replaced = []
    for i in range(len(points)):
        frame = existing[2 * i]
        j = bisect_left(frames, frame - SAME_FRAME)
        if j < len(frames) and frames[j] <= frame + SAME_FRAME:
            replaced.append(i)
    for i in reversed(replaced):
        points.remove(points[i], fast=True)

    # foreach_set sets every point, the ones kept go in as they are
    kept = len(points)
    columns = {}
    for prop in ("co", "handle_left", "handle_right"):
        values = [0.0] * (kept * 2)
        points.foreach_get(prop, values)
        columns[prop] = values
    for prop in ENUMS:
        columns[prop] = get_enums(points, prop)
    for frame, (time, value, interpolation, left, right), (left_frame, left_value, right_frame, right_value) in zip(frames, keys, handles):
        columns["co"] += (frame, value)
        columns["handle_left"] += (left_frame, left_value)
        columns["handle_right"] += (right_frame, right_value)
        columns["interpolation"].append(INTERPOLATION_VALUES[BLENDER_INTERPOLATIONS[interpolation]])
        columns["handle_left_type"].append(FREE_HANDLE)
        columns["handle_right_type"].append(FREE_HANDLE)

    points.add(len(keys))
    for prop, values in columns.items():
        if prop in ENUMS:
            set_enums(points, prop, values)
        else:
            points.foreach_set(prop, values)
    # sorts the keys and recalculates the auto handles of the keys kept, for their new neighbors
    fc.update()

    # update() also moves zero-length FREE handles 0.001 frames off their key, they go back to where they were, the
    # written ones and the ones kept. its sort keeps keys on one frame in order, like sorted()
    count = len(points)
    order = sorted(range(count), key=lambda i: f32(columns["co"][2 * i]))
    co, left, right = [0.0] * (count * 2), [0.0] * (count * 2), [0.0] * (count * 2)
    points.foreach_get("co", co)
    points.foreach_get("handle_left", left)
    points.foreach_get("handle_right", right)
    for position, i in enumerate(order):
        if co[2 * position] != f32(columns["co"][2 * i]):
            continue
        if columns["handle_left_type"][i] == FREE_HANDLE:
            left[2 * position:2 * position + 2] = columns["handle_left"][2 * i:2 * i + 2]
        if columns["handle_right_type"][i] == FREE_HANDLE:
            right[2 * position:2 * position + 2] = columns["handle_right"][2 * i:2 * i + 2]
    points.foreach_set("handle_left", left)
    points.foreach_set("handle_right", right)
    return len(keys)


def write_object(obj, curves, report):
    """Writes one object's curves, each failing curve is reported and skipped. Returns how many keys were written."""
    fps = scene_fps(bpy.context.scene)
    written = 0
    for curve in curves:
        channel = curve.get("channel") if isinstance(curve, dict) else None
        try:
            if not isinstance(channel, str) or not channel:
                raise CurveError("a curve needs a 'channel' id")
            keys = parse_keys(curve.get("keys"))
            if not keys:
                continue
            owner, data_path, array_index = resolve_channel(obj, channel)
            fc, created = get_or_create_fcurve(obj, owner, data_path, array_index)
            if created:
                # held ends (§9.1), existing curves keep theirs
                fc.extrapolation = "CONSTANT"
            written += write_curve(fc, keys, fps)
        except Exception as e:
            report["errors"].append({"object": obj.name, "channel": channel if isinstance(channel, str) else None, "message": str(e) or type(e).__name__})
    return written


def write_keyframes(objects, report):
    """Writes each object's curves. An object that isn't there is reported, as is each curve that fails."""
    for object_name, curves in objects.items():
        obj = bpy.data.objects.get(object_name)
        if obj is None:
            report["missing_objects"].append(object_name)
            continue
        if not isinstance(curves, list):
            report["errors"].append({"object": object_name, "channel": None, "message": "an object's curves must be an array"})
            continue
        written = write_object(obj, curves, report)
        if written:
            report["objects"] += 1
            report["keys"] += written


# how long a write's message stays in Blender's status bar
STATUS_SECONDS = 4


def redraw_status_bar():
    """Blender's status bar only draws again for a few notifiers, renaming an ID is one of them. the scene keeps its
    name, nothing is evaluated again"""
    scene = bpy.context.scene
    scene.name = scene.name


def show_status(text, icon):
    """Shows `text` at the end of Blender's status bar for a few seconds, the next write's message replaces it.
    The draw function is kept in driver_namespace so a later write finds it, even after the add-on was reloaded"""
    namespace = bpy.app.driver_namespace
    previous = namespace.pop("motionkeys_status", None)
    if previous is not None:
        with suppress(Exception):
            bpy.types.STATUSBAR_HT_header.remove(previous)

    def draw(self, context):
        self.layout.label(text=text, icon=icon)

    bpy.types.STATUSBAR_HT_header.append(draw)
    namespace["motionkeys_status"] = draw

    # gone after a while, unless another write replaced it
    def clear():
        if namespace.get("motionkeys_status") is draw:
            del namespace["motionkeys_status"]
            with suppress(Exception):
                bpy.types.STATUSBAR_HT_header.remove(draw)
                redraw_status_bar()
        return None

    bpy.app.timers.register(clear, first_interval=STATUS_SECONDS)
    redraw_status_bar()


def plural(count, word):
    return f"{count:,} {word}{'' if count == 1 else 's'}"


def write(objects, mode):
    """Writes `objects` ({object name: [curve, ...]}) and reports what happened. mode "replace" first removes all of
    each object's animation (and its shape keys'), "add" adds to the curves already there."""
    report = {"objects": 0, "keys": 0, "missing_objects": [], "errors": []}
    if mode not in ("replace", "add"):
        raise ValueError(f"unknown write mode '{mode}'")
    if mode == "replace":
        clean_all_keyframes(objects.keys())
    write_keyframes(objects, report)

    # how it went, in Blender's status bar too
    with suppress(Exception):
        if report["errors"] or report["missing_objects"]:
            show_status("MotionKeys: couldn't write keyframes", "ERROR")
        else:
            show_status(f"MotionKeys: wrote {plural(report['keys'], 'keyframe')} to {plural(report['objects'], 'object')}", "INFO")
    return report
