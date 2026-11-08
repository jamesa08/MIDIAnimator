# what the bridge answers MotionKeys' requests with (docs/bridge_protocol.md). each handler gets the request's body and
# returns the reply's type and body, an exception becomes an error reply. they run on Blender's main thread

import json
import traceback

import bpy

from . import tracker, writer
from .animation import object_curves, scene_fps, source_key_count


def id_list(body, field, required=True):
    """A request's list of ids, raises when it isn't one."""
    value = body.get(field)
    if value is None and not required:
        return []
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise ValueError(f"'{field}' must be an array of object ids")
    return value


def get_scene(body):
    """The scene the user is working in, the same one the tracker's updates send."""
    return "scene", tracker.build_scene()


def watch(body):
    """The curves of the objects to fetch, and the source key count of the ones over the limit (those aren't sent).
    The tracker then watches every object asked for except those over the limit, and sends their curves when their
    keys change."""
    objects = id_list(body, "objects")
    fetch = id_list(body, "fetch")
    approved = set(id_list(body, "approved", required=False))
    limit = body.get("limit")
    if limit is not None and (isinstance(limit, bool) or not isinstance(limit, (int, float))):
        raise ValueError("'limit' must be an integer or null")

    scene = bpy.context.scene
    fps = scene_fps(scene)
    watched = set(objects)
    fetched = set()
    reply = {"objects": {}, "over_limit": {}, "missing_objects": []}
    for name in fetch:
        # only objects that are watched are fetched, each one once
        if name not in watched or name in fetched:
            continue
        fetched.add(name)
        obj = scene.objects.get(name)
        if obj is None:
            reply["missing_objects"].append(name)
            continue
        count = source_key_count(obj)
        if limit is not None and count > limit and name not in approved:
            reply["over_limit"][name] = count
            continue
        reply["objects"][name] = object_curves(obj, fps)

    tracker.set_watched_objects([name for name in objects if name not in reply["over_limit"]], reply["objects"])
    return "curves", reply


def write(body):
    """Writes the keys, the report says what was written and what couldn't be."""
    mode = body.get("mode")
    if mode not in ("replace", "add"):
        raise ValueError(f"unknown write mode {mode!r}, it's \"replace\" or \"add\"")
    objects = body.get("objects")
    if not isinstance(objects, dict):
        raise ValueError("'objects' must be an object of object ids to arrays of curves")
    return "write_result", writer.write(objects, mode)


def run_script(body):
    """Runs a Python script MotionKeys sends. The script defines `execute(args)` (or `execute()`), its return value is
    the result and has to be JSON. `bpy` is there without importing it."""
    language = body.get("language")
    if language != "python":
        raise ValueError(f"the Blender bridge runs Python scripts, not '{language}'")
    namespace = {"bpy": bpy, "__name__": "motionkeys_script"}
    exec(compile(body.get("source", ""), "<motionkeys script>", "exec"), namespace)
    execute = namespace.get("execute")
    if not callable(execute):
        raise ValueError("the script has no execute() function")
    args = body.get("args")
    result = execute(args) if execute.__code__.co_argcount else execute()
    return "script_result", {"result": result}


HANDLERS = {
    "get_scene": get_scene,
    "watch": watch,
    "write": write,
    "run_script": run_script,
}


def answer(server, request):
    """Answers one request from MotionKeys, an error reply when it fails ("failed") or the bridge doesn't know the
    request ("unknown_type")."""
    kind = request.get("type")
    handler = HANDLERS.get(kind)
    if handler is None:
        reply_type, data = "error", json.dumps({"code": "unknown_type", "message": f"the Blender bridge doesn't answer '{kind}' requests"})
    else:
        try:
            reply_type, body = handler(request.get("body") or {})
            # a script's result that isn't JSON fails here, not halfway through sending
            data = json.dumps(body)
        except Exception as e:
            reply_type, data = "error", json.dumps({"code": "failed", "message": str(e) or type(e).__name__, "detail": traceback.format_exc()})
    try:
        server.send_raw(reply_type, data, reply_to=request.get("id"))
    except Exception as e:
        print(f"MotionKeys couldn't answer {kind}: {e!r}")
