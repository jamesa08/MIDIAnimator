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

def validateFCurves(noteOnFCurves, noteOffFCurves, haveSorted: bool=False) -> bool:
    """
    This function will ensure both lists of FCurves have the same data_paths and array_indexes.
    `haveSorted` will sort the objects for you. Default is false. 
    Returns True if valid, False otherwise
    """
    # if there is no FCurves on each object, this is valid
    if (len(noteOnFCurves) == 0 and bool(noteOffFCurves) is False) or (len(noteOffFCurves) == 0 and bool(noteOnFCurves is False)): return True

    # if one doesn't exist, its valid (since there isn't another pair to match with)
    if (bool(noteOnFCurves) is False and bool(noteOffFCurves) is True) or (bool(noteOffFCurves) is False and bool(noteOnFCurves) is True): return True
    
    # if they both don't exist, its invalid
    if bool(noteOnFCurves) is False and bool(noteOffFCurves) is False: return False
    
    # if they don't match in length, immediately stop checking
    if len(noteOnFCurves) != len(noteOffFCurves): return False
    
    # check through FCurves
    if haveSorted:
        iterObj = zip(sorted(noteOnFCurves, key=lambda x: f"{x.data_path}_{x.array_index}"), sorted(noteOffFCurves, key=lambda y: f"{y.data_path}_{y.array_index}"))
    else:
        iterObj = zip(noteOnFCurves, noteOffFCurves)
    
    for noteOnCurve, noteOffCurve in iterObj:
        if noteOnCurve.data_path != noteOffCurve.data_path: return False
        if noteOnCurve.array_index != noteOffCurve.array_index: return False
        
    return True

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

def setKeyframeInterpolation(obj: bpy.types.Object, interpolation: str, data_path=None, array_index=None):
    with suppress(AttributeError):
        if obj is not None and obj.animation_data is not None and obj.animation_data.action is not None:
            for fCrv in FCurvesFromObject(obj):
                if (data_path is None or fCrv.data_path == data_path) and (array_index is None or fCrv.array_index == array_index):
                    fCrv.keyframe_points[-1].interpolation = interpolation

def setKeyframeHandleType(obj: bpy.types.Object, handleType, data_path=None, array_index=None):
    with suppress(AttributeError):
        if obj is not None and obj.animation_data is not None and obj.animation_data.action is not None:
            for fCrv in FCurvesFromObject(obj):
                if (data_path is None or fCrv.data_path == data_path) and (array_index is None or fCrv.array_index == array_index):
                    fCrv.keyframe_points[-1].handle_left_type = handleType
                    fCrv.keyframe_points[-1].handle_right_type = handleType

def copyKeyframeProperties(obj: bpy.types.Object, keyframeToCopy: bpy.types.Keyframe, data_path=None, array_index=None):
    """Copies all properties of the last keyframe on the specified data path and array index EXCEPT for the frame and value."""
    with suppress(AttributeError):
        if obj is not None and obj.animation_data is not None and obj.animation_data.action is not None:
            for fCrv in FCurvesFromObject(obj):
                if (data_path is None or fCrv.data_path == data_path) and (array_index is None or fCrv.array_index == array_index):
                    oldKeyframe = fCrv.keyframe_points[-1]
                    oldFrame = oldKeyframe.co[0]
                    newFrame = keyframeToCopy.co[0]
                    oldKeyframe.interpolation = keyframeToCopy.interpolation
                    oldKeyframe.handle_left[0] = (keyframeToCopy.handle_left[0] - newFrame) + oldFrame
                    oldKeyframe.handle_left[1] = keyframeToCopy.handle_left[1]
                    oldKeyframe.handle_right[0] = (keyframeToCopy.handle_right[0] - newFrame) + oldFrame
                    oldKeyframe.handle_right[1] = keyframeToCopy.handle_right[1]
                    oldKeyframe.handle_left_type = keyframeToCopy.handle_left_type
                    oldKeyframe.handle_right_type = keyframeToCopy.handle_right_type


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
