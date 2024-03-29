import bpy
from contextlib import suppress
from mathutils import Vector
from mathutils.bvhtree import BVHTree
from typing import Any, Tuple, List, Union, Set

def shapeKeysFromObject(obj: bpy.types.Object) -> Tuple[List[bpy.types.ShapeKey], bpy.types.ShapeKey]:
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

def FCurvesFromObject(obj) -> List[bpy.types.FCurve]:
    if obj.animation_data is None: return []
    if obj.animation_data.action is None: return []
    
    return list(obj.animation_data.action.fcurves)

def shapeKeyFCurvesFromObject(obj) -> List[bpy.types.FCurve]:
    with suppress(AttributeError):
        if obj.data.shape_keys is None: return []
        if obj.data.shape_keys.animation_data.action is None: return []
        
        return list(obj.data.shape_keys.animation_data.action.fcurves)
    return []

def cleanKeyframes(obj, channels: Set={"all_channels"}):
    fCurves = FCurvesFromObject(obj)
    
    for fCurve in fCurves:
        if {fCurve.data_path, "all_channels"}.intersection(channels):
            obj.animation_data.action.fcurves.remove(fCurve)
    
    for fCurve in shapeKeyFCurvesFromObject(obj):
        obj.data.shape_keys.animation_data.action.fcurves.remove(fCurve)

def secToFrames(sec: float) -> float:
    bScene = bpy.context.scene
    fps = bScene.render.fps / bScene.render.fps_base
    
    return sec * fps

def framesToSec(frames: float) -> float:
    bScene = bpy.context.scene
    fps = bScene.render.fps / bScene.render.fps_base
    
    return frames / fps

def getExactFps() -> float:
    bScene = bpy.context.scene
    return bScene.render.fps / bScene.render.fps_base

def cleanCollection(col: bpy.types.Collection, refObject: bpy.types.Object=None) -> None:
    """
    Cleans a collection of old objects (to be reanimated)
    """ 

    objsToRemove = [obj for obj in col.all_objects if obj != refObject]

    for obj in objsToRemove:
        bpy.data.objects.remove(obj, do_unlink=True)

def setKeyframeInterpolation(obj: bpy.types.Object, interpolation: str, data_path=None):
    with suppress(AttributeError):
        if obj is not None and obj.animation_data is not None and obj.animation_data.action is not None:
            for fCrv in FCurvesFromObject(obj):
                if data_path is None or fCrv.data_path == data_path:
                    fCrv.keyframe_points[-1].interpolation = interpolation

def setKeyframeHandleType(obj: bpy.types.Object, handleType, data_path=None):
    with suppress(AttributeError):
        if obj is not None and obj.animation_data is not None and obj.animation_data.action is not None:
            for fCrv in FCurvesFromObject(obj):
                if data_path is None or fCrv.data_path == data_path:
                    fCrv.keyframe_points[-1].handle_left_type = handleType
                    fCrv.keyframe_points[-1].handle_right_type = handleType

def deleteMarkers(name: str):
    scene = bpy.context.scene
    for marker in scene.timeline_markers:
        if name in marker.name:
            scene.timeline_markers.remove(marker)

# This method could be useful to calculate the distance between 2 objects (think: drumsticks?)
def distanceFromVectors(point1: Vector, point2: Vector) -> float: 
    """Calculate distance between two points.""" 
    
    return (point2 - point1).length

def velocityFromVectors(point1: Vector, point2: Vector, frames: float) -> float:
    """Calculates velocity from 2 vectors given a time"""
    
    distance = distanceFromVectors(point1, point2)
    if frames != 0:
        return distance / framesToSec(frames)
    
    return 0

def timeFromVectors(point1: Vector, point2: Vector, velocity: float) -> float:
    distance = distanceFromVectors(point1, point2)
    return distance / velocity

def showHideObj(obj: bpy.types.Object, hide: bool, frame: int):
    obj.hide_viewport = hide
    obj.hide_render = hide
    obj.keyframe_insert(data_path="hide_viewport", frame=frame)
    obj.keyframe_insert(data_path="hide_render", frame=frame)

def worldBoundingBox(obj: bpy.types.Object):
    """returns the corners of the bounding box of an object in world coordinates"""
    return [obj.matrix_world @ Vector(corner) for corner in obj.bound_box]

def objectsOverlap(obj1: bpy.types.Object, obj2: bpy.types.Object) -> bool:
    """returns True if the object's bounding boxes are overlapping"""
    vert1 = worldBoundingBox(obj1)
    vert2 = worldBoundingBox(obj2)
    faces = [(0, 1, 2, 3), (4, 7, 6, 5), (0, 4, 5, 1), (1, 5, 6, 2), (2, 6, 7, 3), (4, 0, 3, 7)]

    bvh1 = BVHTree.FromPolygons(vert1, faces)
    bvh2 = BVHTree.FromPolygons(vert2, faces)

    return bool(bvh1.overlap(bvh2))
