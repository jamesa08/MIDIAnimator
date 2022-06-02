import bpy
from mathutils import Vector  
from typing import Any, Tuple, List, Union


def FCurvesFromObject(obj) -> list:
    if obj.animation_data.action is None:
        raise RuntimeError("Object has no FCurves!")  # maybe this should return an empty list instead of crashing? & soft warn?
    
    return list(obj.animation_data.action.fcurves)

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


def insertKeyframe(object: bpy.types.Object, dataPath: str, value: Any, frame: Union[int, float]):
    #TODO: implement this correctly
    if dataPath.split(".")[1] == "shape_keys":
        try:
            assert isinstance(value, "float") or isinstance(value, "int")
            exec(f"{object}.{dataPath}.value = {value}")
            exec(f"{object}.{dataPath}.keyframe_insert(data_path='value', frame={frame})")
        except Exception as e:
            raise RuntimeError(f"Error caught! '{e}'")
    # bpy.context.active_object.data.shape_keys.key_blocks['Vibrate'].keyframe_insert(data_path="value")


# This method could be useful to calculate the distance between 2 objects (think: drumsticks?)
def distanceFromVectors(point1: Vector, point2: Vector) -> float: 
    """Calculate distance between two points.""" 
    
    return (point2 - point1).length
