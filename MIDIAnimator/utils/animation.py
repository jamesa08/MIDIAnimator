import bpy


def FCurvesFromObject(bObj) -> list:
    if bObj.animation_data.action is None:
        raise RuntimeError("Object has no FCurves!")  # maybe this should return an empty list instead of crashing? & soft warn?
    
    return list(bObj.animation_data.action.fcurves)


def FCurveLength(FCurveName: str, index: int) -> float:
    bObj = bpy.data.objects[FCurveName]
    FCurveKeyframes = FCurvesFromObject(bObj)[index].keyframe_points
    
    # FCurveKeyframes gets all the keyframes in a FCurve
    # co is a (x, y) pair of a specific keyframe point 
    start, end = FCurveKeyframes[0].co[0], FCurveKeyframes[-1].co[0]
    
    return end - start

def secToFrames(sec: float) -> float:
    bScene = bpy.context.scene
    fps = bScene.render.fps / bScene.render.fps_base
    
    return sec * fps
    

def getExactFps() -> float:
    bScene = bpy.context.scene
    return bScene.render.fps / bScene.render.fps_base


def shapeKeysFromObject(object):
    # from Animation Nodes
    # https://github.com/JacquesLucke/animation_nodes/blob/7a74e31fca0e7fce6edefdb8183dc4ac9c5acbfc/animation_nodes/nodes/shape_key/shape_keys_from_object.py
    if object is None: return [], None
    if object.type not in ("MESH", "CURVE", "LATTICE"): return [], None
    if object.data.shape_keys is None: return [], None

    reference = object.data.shape_keys.reference_key
    return list(object.data.shape_keys.key_blocks)[1:], reference


