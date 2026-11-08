# reading Blender's animation: F-curves of objects and their shape keys, as the bridge's curves (docs/bridge_protocol.md
# §8.6): keys in seconds, Bezier, linear or constant, with absolute handles

import math
import struct

import bpy
from bpy_extras import anim_utils

# action slots came in Blender 4.4. from then on the old action.fcurves API only reaches a legacy slot the object isn't
# assigned to, so keys written through it never animate, and reading it can miss the slot the object uses
SLOTTED_ACTIONS = bpy.app.version >= (4, 4, 0)


def scene_fps(scene):
    """The scene's frame rate with its base (29.97 is 30 / 1.001), the same one the writer converts with."""
    return scene.render.fps / scene.render.fps_base


def fcurves_of(id_data):
    """The F-curves of an object's or shape keys' action, [] without one."""
    anim_data = id_data.animation_data if id_data else None
    if anim_data is None or anim_data.action is None:
        return []
    if not SLOTTED_ACTIONS:
        return list(anim_data.action.fcurves)
    channelbag = anim_utils.action_get_channelbag_for_slot(anim_data.action, anim_data.action_slot)
    return list(channelbag.fcurves) if channelbag else []


def shape_keys_of(obj):
    """The datablock that holds an object's shape keys and their animation, None without shape keys."""
    return getattr(obj.data, "shape_keys", None) if obj is not None else None


def object_fcurves(obj):
    """Every F-curve that animates an object: its own and its shape keys' (data paths like `key_blocks["Smile"].value`)."""
    return fcurves_of(obj) + fcurves_of(shape_keys_of(obj))


def source_key_count(obj):
    """The keys Blender stores for an object over all its F-curves, before any conversion: what the import limit counts."""
    return sum(len(fcurve.keyframe_points) for fcurve in object_fcurves(obj))


# channel ids

def resolve(owner, data_path):
    """The value at `data_path` on `owner`, or raises ValueError when there's nothing there."""
    return owner.path_resolve(data_path)


def is_scalar(value):
    """A single value (a number, or an enum's name), not a vector, color or array property."""
    return isinstance(value, (bool, int, float, str))


def is_array(value):
    """A vector, color or array property, whose F-curves each animate one component."""
    return not is_scalar(value) and hasattr(value, "__len__")


def channel_of(owner, fcurve):
    """An F-curve's channel id (channel_id) and whether its data path resolves on `owner`."""
    data_path, index = fcurve.data_path, fcurve.array_index
    try:
        value = resolve(owner, data_path)
    except (ValueError, TypeError, AttributeError):
        return f"{data_path}[{index}]", False
    if is_scalar(value) and index == 0:
        return data_path, True
    return f"{data_path}[{index}]", True


def channel_id(owner, fcurve):
    """The channel id of an F-curve: its data path for a property that isn't an array (`key_blocks["Smile"].value`,
    `["glow"]`), the data path and index for one component of an array (`location[2]`, `["color"][1]`). A path that
    doesn't resolve keeps its index."""
    return channel_of(owner, fcurve)[0]


# the channels an object lists (docs/bridge_protocol.md §8.5)

# names and axes of the transform properties of objects and bones, the name leaves the axis out. other data paths are
# sent raw, named by their channel id
CHANNEL_LABELS = {
    "location": ("Location", "XYZ"),
    "rotation_euler": ("Rotation", "XYZ"),
    "rotation_quaternion": ("Quaternion", "WXYZ"),
    "rotation_axis_angle": ("Axis-Angle", "WXYZ"),
    "scale": ("Scale", "XYZ"),
    "delta_location": ("Delta Location", "XYZ"),
    "delta_rotation_euler": ("Delta Rotation", "XYZ"),
    "delta_rotation_quaternion": ("Delta Quaternion", "WXYZ"),
    "delta_scale": ("Delta Scale", "XYZ"),
}

# the labels channels are listed under, apart from bones (their names)
OBJECT_GROUP = "Object"
SHAPE_KEYS_GROUP = "Shape Keys"
CUSTOM_PROPERTIES_GROUP = "Custom Properties"

# what the object's own transform channels move, for MotionKeys' 3D preview: rotation_euler[2] is rotation.z. bones,
# delta transforms and axis-angle rotations have none
SEMANTICS = {"location": "position", "rotation_euler": "rotation", "rotation_quaternion": "quaternion", "scale": "scale"}

# the rotation property of each rotation mode that isn't Euler
ROTATION_PATHS = {"QUATERNION": "rotation_quaternion", "AXIS_ANGLE": "rotation_axis_angle"}


def transform_paths(rotation_mode):
    """The transform properties an object or bone in this rotation mode is moved by."""
    return ("location", ROTATION_PATHS.get(rotation_mode, "rotation_euler"), "scale")


def make_channel(channel, name, group, axis=None, semantic=None):
    """A channel as the scene sends it, `animated` once an F-curve with keys is found for it."""
    listed = {"id": channel, "name": name, "group": group}
    if axis:
        listed["axis"] = axis
    if semantic:
        listed["semantic"] = semantic
    listed["animated"] = False
    return listed


def labeled_channel(channel, prop, index, group):
    """A channel of a transform property in CHANNEL_LABELS with its name and axis, any other one raw."""
    name, axes = CHANNEL_LABELS.get(prop, (None, ""))
    if index < len(axes):
        return make_channel(channel, name, group, axes[index])
    return make_channel(channel, channel, group)


def custom_property_paths(struct):
    """The data path of each custom property of an object or bone, and its key: {'["glow"]': "glow"}."""
    return {f'["{bpy.utils.escape_identifier(key)}"]': key for key in struct.keys()}


def bone_prefix(data_path):
    """The `pose.bones["Arm"]` a data path starts with, None for a path that isn't on a bone."""
    if not data_path.startswith('pose.bones["'):
        return None
    # the bone's name ends at the first quote that isn't escaped
    i = len('pose.bones["')
    while i < len(data_path):
        if data_path[i] == "\\":
            i += 2
            continue
        if data_path[i] == '"':
            return data_path[:i + 2] if data_path[i:i + 2] == '"]' else None
        i += 1
    return None


def object_channels(obj):
    """The channels MotionKeys can write to on an object, in order: the object's own (its transform, then its other
    animated F-curves), its shape keys, its animated custom properties, then each bone's transform and other animated
    F-curves. Every F-curve with keys is listed, animated, under the id its curve is sent with (object_curves). The
    object's group comes first, MotionKeys leaves the first channel's group out of its labels."""
    shape_keys = shape_keys_of(obj)
    by_id = {}
    own, shapes, custom = [], [], []
    # each bone and its channels, by the `pose.bones["Arm"]` their data paths start with
    bones = {}

    def add(section, channel):
        by_id[channel["id"]] = channel
        section.append(channel)

    for prop in transform_paths(obj.rotation_mode):
        name, axes = CHANNEL_LABELS[prop]
        for i, axis in enumerate(axes):
            semantic = f"{SEMANTICS[prop]}.{axis.lower()}" if prop in SEMANTICS else None
            add(own, make_channel(f"{prop}[{i}]", name, OBJECT_GROUP, axis, semantic))

    if shape_keys is not None:
        # the first one is the reference key, the others are blended onto it
        for block in shape_keys.key_blocks[1:]:
            add(shapes, make_channel(f'key_blocks["{bpy.utils.escape_identifier(block.name)}"].value', block.name, SHAPE_KEYS_GROUP))

    if obj.pose is not None:
        for bone in obj.pose.bones:
            prefix = f'pose.bones["{bpy.utils.escape_identifier(bone.name)}"]'
            section = []
            bones[prefix] = (bone, section)
            for prop in transform_paths(bone.rotation_mode):
                name, axes = CHANNEL_LABELS[prop]
                for i, axis in enumerate(axes):
                    add(section, make_channel(f"{prefix}.{prop}[{i}]", name, bone.name, axis))

    # the transforms listed so far are arrays, their F-curves' ids need no lookup
    transforms = set(by_id)
    # the custom property keys of the object and of each bone, read once an F-curve animates one
    property_keys = {}

    def property_name(struct, path, channel):
        """A scalar custom property's key, None for anything else (an array's component)."""
        if path != channel:
            return None
        pointer = struct.as_pointer()
        if pointer not in property_keys:
            property_keys[pointer] = custom_property_paths(struct)
        return property_keys[pointer].get(path)

    def unlisted(on_object, data_path, index, channel, resolved):
        """The section and channel of an animated F-curve that isn't listed yet. Anything on the shape keys' action is
        raw under Shape Keys, and a path that doesn't resolve is raw under the object's group, so its curve can still
        be read."""
        if not on_object:
            return shapes, make_channel(channel, channel, SHAPE_KEYS_GROUP)
        if not resolved:
            return own, make_channel(channel, channel, OBJECT_GROUP)
        if data_path.startswith("["):
            return custom, make_channel(channel, property_name(obj, data_path, channel) or channel, CUSTOM_PROPERTIES_GROUP)
        prefix = bone_prefix(data_path)
        if prefix in bones:
            bone, section = bones[prefix]
            rest = data_path[len(prefix):]
            if rest.startswith("["):
                return section, make_channel(channel, property_name(bone, rest, channel[len(prefix):]) or channel, bone.name)
            return section, labeled_channel(channel, rest[1:], index, bone.name)
        return own, labeled_channel(channel, data_path, index, OBJECT_GROUP)

    for owner in (obj, shape_keys):
        for fcurve in fcurves_of(owner):
            if not len(fcurve.keyframe_points):
                continue
            channel = f"{fcurve.data_path}[{fcurve.array_index}]"
            if channel not in transforms:
                channel, resolved = channel_of(owner, fcurve)
                if channel not in by_id:
                    add(*unlisted(owner is obj, fcurve.data_path, fcurve.array_index, channel, resolved))
            by_id[channel]["animated"] = True

    return own + shapes + custom + [channel for bone, section in bones.values() for channel in section]


# keys

# Blender's interpolations the format has, the others are converted
WIRE_INTERPOLATIONS = {"BEZIER": "bezier", "LINEAR": "linear", "CONSTANT": "constant"}
# and back, for the fitted pieces
WIRE_NAMES = {wire: identifier for identifier, wire in WIRE_INTERPOLATIONS.items()}

# a keyframe enum's values by identifier, foreach_get and foreach_set read and write enums as their values
KEYFRAME_ENUMS = {prop: {item.identifier: item.value for item in bpy.types.Keyframe.bl_rna.properties[prop].enum_items} for prop in ("interpolation", "easing", "handle_left_type", "handle_right_type")}
INTERPOLATION_NAMES = {value: identifier for identifier, value in KEYFRAME_ENUMS["interpolation"].items()}

# foreach_get and foreach_set work on enums from Blender 4.3 on (4.0 reads garbage and writes nothing), older versions
# go key by key
ENUM_FOREACH = bpy.app.version >= (4, 3, 0)


def get_enums(points, prop):
    """A keyframe enum of every point, as values."""
    if ENUM_FOREACH:
        values = [0] * len(points)
        points.foreach_get(prop, values)
        return values
    by_identifier = KEYFRAME_ENUMS[prop]
    return [by_identifier[getattr(point, prop)] for point in points]


def set_enums(points, prop, values):
    """Sets a keyframe enum of every point, from values."""
    if ENUM_FOREACH:
        points.foreach_set(prop, values)
        return
    identifiers = {value: identifier for identifier, value in KEYFRAME_ENUMS[prop].items()}
    for point, value in zip(points, values):
        setattr(point, prop, identifiers[value])


# a converted span is within 0.1% of its value range, at least 1e-6. Blender evaluates in 32-bit floats, so the
# tolerance is also never below 8 float steps of the span's largest value: the curve itself is only that exact, and
# no fit could get closer
TOLERANCE = 1e-3
TOLERANCE_FLOOR = 1e-6
FLOAT32_FLOOR = 2.0 ** -20
# fits aim at half the tolerance at their samples, so the curve between the samples stays within it
FIT_MARGIN = 0.5
# a fitted piece is checked at this many samples at least, at most this many frames apart
CHECK_SAMPLES = 16
CHECK_SPACING = 0.25
# the value range and turning points of a converted segment are sampled this often, with a cap for very long
# segments (the cap holds for the checks too)
RANGE_SAMPLES = 64
RANGE_SAMPLES_MAX = 4096
# how far into a piece its end slopes are measured, as a share of its length
SLOPE_STEP = 1.0 / 64.0
# pieces are halved at most this often, then the rest is linear
MAX_DEPTH = 24

_float32 = struct.Struct("<f")


def f32(x):
    """`x` rounded to a 32-bit float, the precision Blender evaluates F-curves at and stores keys in."""
    return _float32.unpack(_float32.pack(x))[0]


def read_keys(fcurve):
    """An F-curve's keys in frames, sorted, one per frame, with finite numbers: [frame, value, interpolation
    identifier, left frame, left value, right frame, right value]."""
    points = fcurve.keyframe_points
    count = len(points)
    co, left, right = [0.0] * (count * 2), [0.0] * (count * 2), [0.0] * (count * 2)
    points.foreach_get("co", co)
    points.foreach_get("handle_left", left)
    points.foreach_get("handle_right", right)
    interpolation = get_enums(points, "interpolation")

    keys = []
    for i in range(count):
        frame, value = co[2 * i], co[2 * i + 1]
        if not (math.isfinite(frame) and math.isfinite(value)):
            continue
        key = [frame, value, INTERPOLATION_NAMES.get(interpolation[i], "BEZIER"), left[2 * i], left[2 * i + 1], right[2 * i], right[2 * i + 1]]
        # a broken handle sits on its key
        if not all(math.isfinite(x) for x in key[3:]):
            key[3:] = [frame, value, frame, value]
        keys.append(key)

    # Blender keeps keys sorted after fcurve.update(), a script may not have called it. of several keys on one frame
    # the last one is kept, the segment after the frame starts from it
    keys.sort(key=lambda key: key[0])
    unique = []
    for key in keys:
        if unique and unique[-1][0] == key[0]:
            unique[-1] = key
        else:
            unique.append(key)
    return unique


def converting_modifiers(fcurve):
    """Whether the F-curve has enabled modifiers that change its keyed range. Cycles only repeat the range outside it,
    the curve holds there anyway."""
    return any(not modifier.mute and modifier.type != "CYCLES" for modifier in fcurve.modifiers)


def cubic(p0, p1, p2, p3, u):
    s = 1.0 - u
    return s * s * s * p0 + 3.0 * s * s * u * p1 + 3.0 * s * u * u * p2 + u * u * u * p3


class Fitter:
    """Fits Bezier pieces to Blender's own evaluation of an F-curve, `fcurve.evaluate(frame)`, between two keys."""

    def __init__(self, fcurve):
        self.fcurve = fcurve

    def evaluate(self, frame):
        # frame is already a 32-bit float, Blender evaluates at exactly that frame. a modifier that computes nothing
        # there (a logarithm of a negative number) gives 0, or no piece would ever fit it
        value = self.fcurve.evaluate(frame)
        return value if math.isfinite(value) else 0.0

    def tolerance(self, keys, start, end):
        """The tolerance for the converted span from keys[start] to keys[end]: 0.1% of its value range."""
        values = [key[1] for key in keys[start:end + 1]]
        for i in range(start, end):
            x0, x1 = keys[i][0], keys[i + 1][0]
            samples = min(RANGE_SAMPLES_MAX, max(RANGE_SAMPLES, math.ceil((x1 - x0) / CHECK_SPACING)))
            values.extend(self.evaluate(f32(x0 + (x1 - x0) * k / samples)) for k in range(1, samples))
        largest = max(abs(value) for value in values)
        return max(TOLERANCE * (max(values) - min(values)), TOLERANCE_FLOOR, FLOAT32_FLOOR * largest)

    def end_slopes(self, x0, y0, x1, y1):
        """The curve's slope just after x0 and just before x1, from a parabola through each end and two samples
        inside the piece."""
        d = x1 - x0

        def slope(x, y, direction):
            xa, xb = f32(x + direction * d * SLOPE_STEP), f32(x + direction * d * SLOPE_STEP * 2.0)
            p, q = xa - x, xb - x
            if p == 0.0 or q == 0.0 or p == q:
                return (y1 - y0) / d
            ya, yb = self.evaluate(xa) - y, self.evaluate(xb) - y
            return (ya * q * q - yb * p * p) / (p * q * (q - p))

        return slope(x0, y0, 1.0), slope(x1, y1, -1.0)

    def segment(self, x0, y0, x1, y1, target):
        """Pieces from (x0, y0) to (x1, y1) within `target` of the evaluated curve at their samples:
        [(end frame, end value, interpolation, right handle of the start, left handle of the end)], handles in frames,
        None for linear and constant pieces. Keys go at the curve's turning points first, then each piece between
        them is halved until it fits."""
        pieces = []
        start = (x0, y0)
        for frame in self.turning_points(x0, y0, x1, y1, target) + [None]:
            end = (x1, y1) if frame is None else (frame, self.evaluate(frame))
            self.fit(start[0], start[1], end[0], end[1], target, 0, pieces)
            start = end
        return pieces

    def turning_points(self, x0, y0, x1, y1, target):
        """Frames strictly between x0 and x1 where the curve turns back by more than `target`: an elastic's peaks, a
        bounce's bounces, a noise's crests. The pieces between them only rise or fall, and fit with fewer keys."""
        count = min(RANGE_SAMPLES_MAX, max(RANGE_SAMPLES, math.ceil((x1 - x0) / CHECK_SPACING)))
        xs = [x0] + [f32(x0 + (x1 - x0) * k / count) for k in range(1, count)] + [x1]
        ys = [y0] + [self.evaluate(x) for x in xs[1:-1]] + [y1]

        # the extreme sample since the last turn, a turn once the curve came back from it by more than target
        turns = []
        direction, candidate = 0, 0
        for i in range(1, len(xs)):
            y = ys[i]
            if direction == 0:
                if abs(y - ys[0]) > target:
                    direction, candidate = (1 if y > ys[0] else -1), i
            elif (y - ys[candidate]) * direction >= 0:
                candidate = i
            elif (ys[candidate] - y) * direction > target:
                turns.append((candidate, direction))
                direction, candidate = -direction, i

        frames = []
        for i, direction in turns:
            frame = self.extreme(xs[i - 1], xs[i + 1], direction)
            if (frames[-1] if frames else x0) < frame < x1:
                frames.append(frame)
        return frames

    def extreme(self, a, b, direction):
        """The frame of the curve's highest point in [a, b] (lowest for direction -1), by golden-section search. It
        finds a bounce's corner as well as a smooth peak."""
        ratio = (math.sqrt(5.0) - 1.0) / 2.0
        c, d = f32(b - ratio * (b - a)), f32(a + ratio * (b - a))
        fc, fd = direction * self.evaluate(c), direction * self.evaluate(d)
        for _ in range(64):
            if not a < c < d < b:
                break
            if fc > fd:
                b, d, fd = d, c, fc
                c = f32(b - ratio * (b - a))
                fc = direction * self.evaluate(c)
            else:
                a, c, fc = c, d, fd
                d = f32(a + ratio * (b - a))
                fd = direction * self.evaluate(d)
        return c if fc > fd else d

    def jump(self, lo, hi, y0, target):
        """The first frame after `lo` where the curve isn't within `target` of y0 any more, `hi` is past it. Blender's
        curves take their new value at a jump, so that's the frame of the jump."""
        while True:
            middle = f32((lo + hi) / 2.0)
            if not lo < middle < hi:
                return hi
            if abs(self.evaluate(middle) - y0) <= target:
                lo = middle
            else:
                hi = middle

    def fit(self, x0, y0, x1, y1, target, depth, pieces):
        d = x1 - x0
        # one Bezier piece, handles a third of the way along with the curve's slopes (the one-third rule, §8.6), so
        # time runs linearly along it and u is (x - x0) / d
        m0, m1 = self.end_slopes(x0, y0, x1, y1)
        h1 = (x0 + d / 3.0, y0 + m0 * d / 3.0)
        h2 = (x1 - d / 3.0, y1 - m1 * d / 3.0)

        # the Bezier piece, else a straight one (where the slopes measured inside a tiny piece are off), else a step:
        # flat at y0, then flat at y1 after a jump (a constant key, a stepped curve), checked at the samples together.
        # the samples stop once none of them fits
        bezier = linear = step = True
        # the frames around the jump: the last sample at y0 and the first at y1
        before, after = x0, None
        count = min(RANGE_SAMPLES_MAX, max(CHECK_SAMPLES, math.ceil(d / CHECK_SPACING)))
        for k in range(1, count):
            x = f32(x0 + d * k / count)
            if not x0 < x < x1:
                continue
            y = self.evaluate(x)
            u = (x - x0) / d
            bezier = bezier and abs(cubic(y0, h1[1], h2[1], y1, u) - y) <= target
            linear = linear and abs(y0 + (y1 - y0) * u - y) <= target
            if step and after is None:
                if abs(y - y0) <= target:
                    before = x
                elif abs(y - y1) <= target:
                    after = x
                else:
                    step = False
            elif step:
                step = abs(y - y1) <= target
            if not (bezier or linear or step):
                break
        if bezier:
            pieces.append((x1, y1, "bezier", h1, h2))
            return
        if linear:
            pieces.append((x1, y1, "linear", None, None))
            return
        if step:
            # a constant piece up to the jump, found to the frame's last bit, then the rest after it
            jump = self.jump(before, after if after is not None else x1, y0, target)
            if jump >= x1:
                pieces.append((x1, y1, "constant", None, None))
                return
            y_jump = self.evaluate(jump)
            pieces.append((jump, y_jump, "constant", None, None))
            self.fit(jump, y_jump, x1, y1, target, depth + 1, pieces)
            return

        middle = f32(x0 + d / 2.0)
        if depth >= MAX_DEPTH or not (x0 < middle < x1):
            # too small to halve again, as close as 32-bit frames get
            pieces.append((x1, y1, "linear", None, None))
            return
        y_middle = self.evaluate(middle)
        self.fit(x0, y0, middle, y_middle, target, depth + 1, pieces)
        self.fit(middle, y_middle, x1, y1, target, depth + 1, pieces)


def convert_keys(fcurve, keys):
    """The keys with every segment the format can't express replaced by fitted ones. Segments with Bezier, linear or
    constant interpolation pass through, unless modifiers change the curve: then the whole keyed range is fitted.
    Original keys stay where they are."""
    modified = converting_modifiers(fcurve)
    fitter = Fitter(fcurve)
    if modified:
        # the curve as Blender evaluates it, modifiers included, also at the keys. their handles move with them
        for key in keys:
            value = fitter.evaluate(key[0])
            key[4] += value - key[1]
            key[6] += value - key[1]
            key[1] = value

    converted = [modified or key[2] not in WIRE_INTERPOLATIONS for key in keys[:-1]]
    out = [keys[0]]
    start = 0
    while start < len(keys) - 1:
        if not converted[start]:
            out.append(keys[start + 1])
            start += 1
            continue
        # a run of converted segments shares the tolerance of its whole span
        end = start
        while end < len(keys) - 1 and converted[end]:
            end += 1
        target = fitter.tolerance(keys, start, end) * FIT_MARGIN
        for i in range(start, end):
            a, b = out[-1], keys[i + 1]
            for x1, y1, interpolation, h1, h2 in fitter.segment(a[0], a[1], b[0], b[1], target):
                a[2] = WIRE_NAMES[interpolation]
                if h1 is not None:
                    a[5], a[6] = h1
                key = b if x1 == b[0] else [x1, y1, None, x1, y1, x1, y1]
                if h2 is not None:
                    key[3], key[4] = h2
                out.append(key)
                a = key
        start = end
    return out


def fcurve_keys(fcurve, fps):
    """An F-curve's keys on the wire: [time, value, interpolation] or with the handles,
    [time, value, interpolation, left time, left value, right time, right value], in seconds."""
    keys = read_keys(fcurve)
    if not keys:
        return []
    keys = convert_keys(fcurve, keys)

    wire = []
    previous_bezier = False
    for key in keys:
        # the last key's interpolation leads nowhere, an easing there is sent as Bezier with its handles
        interpolation = WIRE_INTERPOLATIONS.get(key[2], "bezier")
        if interpolation == "bezier" or previous_bezier:
            numbers = [key[0] / fps, key[1], key[3] / fps, key[4], key[5] / fps, key[6]]
        else:
            numbers = [key[0] / fps, key[1]]
        # every number is finite (§8.6), a slope that overflowed doesn't make the whole message unreadable
        if not all(math.isfinite(x) for x in numbers):
            continue
        wire.append(numbers[:2] + [interpolation] + numbers[2:])
        previous_bezier = interpolation == "bezier"
    return wire


def object_curves(obj, fps):
    """An object's curves on the wire, its own F-curves and its shape keys', one per channel that has keys."""
    curves = []
    channels = set()
    for owner in (obj, shape_keys_of(obj)):
        for fcurve in fcurves_of(owner):
            if not len(fcurve.keyframe_points):
                continue
            channel = channel_id(owner, fcurve)
            if channel in channels:
                continue
            channels.add(channel)
            curves.append({"channel": channel, "keys": fcurve_keys(fcurve, fps)})
    return curves
