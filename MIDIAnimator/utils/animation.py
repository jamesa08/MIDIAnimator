import bpy
from mathutils import Vector  
from typing import Tuple, List


animationTypes = ("projectile", "string")

def FCurvesFromObject(obj) -> list:
    if obj.animation_data.action is None:
        raise RuntimeError("Object has no FCurves!")  # maybe this should return an empty list instead of crashing? & soft warn?
    
    return list(obj.animation_data.action.fcurves)

# TIL FCurves has a built-in range() method, returns a type Vector with first value being start frame, and last value being end frame 
def FCurveLength(obj: str, index: int, tup=False):
    """returns the FCurve given the blender object

    :param str FCurveName: blender object
    :param int index: index of the FCurve, which FCurve to return
    :param bool tup: returns a tuple of the start and end time, defaults to False
    :return: either a tuple of the start and end time (if tup is True), or a float of the length of the FCurve
    """
    FCurveKeyframes = FCurvesFromObject(obj)[index].keyframe_points
    
    # FCurveKeyframes gets all the keyframes in a FCurve
    # co is a (x, y) pair of a specific keyframe point 
    start, end = FCurveKeyframes[0].co[0], FCurveKeyframes[-1].co[0]
    
    if tup:
        return (start, end)
    
    return end - start

def secToFrames(sec: float) -> float:
    bScene = bpy.context.scene
    fps = bScene.render.fps / bScene.render.fps_base
    
    return sec * fps
    

def getExactFps() -> float:
    bScene = bpy.context.scene
    return bScene.render.fps / bScene.render.fps_base


def shapeKeysFromObject(object: bpy.types.Object) -> Tuple[List[bpy.types.ShapeKey], bpy.types.ShapeKey]:
    """gets shape keys from object

    :param bpy.types.Object object: the blender object
    :return tuple: returns a tuple, first element being a list of the shape keys and second element being the reference key
    """
    # from Animation Nodes
    # https://github.com/JacquesLucke/animation_nodes/blob/7a74e31fca0e7fce6edefdb8183dc4ac9c5acbfc/animation_nodes/nodes/shape_key/shape_keys_from_object.py
    
    if object is None: return [], None
    if object.type not in ("MESH", "CURVE", "LATTICE"): return [], None
    if object.data.shape_keys is None: return [], None

    reference = object.data.shape_keys.reference_key
    return list(object.data.shape_keys.key_blocks)[1:], reference


# This method could be useful to calculate the distance between 2 objects (think: drumsticks?)
def distanceFromVectors(point1: Vector, point2: Vector) -> float: 
    """Calculate distance between two points.""" 
    
    return (point2 - point1).length
