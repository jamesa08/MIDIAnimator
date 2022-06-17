import bpy
from contextlib import suppress
from mathutils import Vector  
from typing import Any, Tuple, List, Union, Set

def split_path(data_path):
    '''
    Split a data_path into parts
    '''

    # from Jaroslav Jerryno Novotny
    # https://blender.stackexchange.com/a/42170/23372
    
    if not data_path:
        return []

    # extract names from data_path
    names = data_path.split('"')[1::2]
    data_path_no_names = ''.join(data_path.split('"')[0::2])

    # segment into chunks
    # ID props will be segmented by replacing ][ with ].[
    data_chunks = data_path_no_names.replace('][', '].[').split('.')
    # probably regex should be used here and things put into dictionary
    # so it's clear what chunk is what
    # depends of use case, the main idea is to extract names, segment, then put back

    # put names back into chunks where [] are
    for id, chunk in enumerate(data_chunks):
        if chunk.find('[]') > 0:
            data_chunks[id] = chunk.replace('[]', '["' + names.pop(0) + '"]')

    return data_chunks

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

def FCurvesFromObject(obj) -> List[bpy.types.FCurve]:
    if obj.animation_data is None: return []
    if obj.animation_data.action is None: return []
    
    return list(obj.animation_data.action.fcurves)

def shapeKeyFCurvesFromObject(obj) -> List[bpy.types.FCurve]:
    if obj.data.shape_keys is None: return []
    if obj.data.shape_keys.animation_data.action is None: return []
    
    return list(obj.data.shape_keys.animation_data.action.fcurves)

# XXX This function is still a WIP.
def insertKeyframeOnObjects(keyObj, refObj, frame, value):
    # TODO:
    # better names for these variables
    # maybe the actual keyframing part should be handedled by the actual animate() methods themselves?
    # that way they can mutate the value data themselves & evaluate it however they like (such as offsetting objects with locations)
     
    fCurves = FCurvesFromObject(refObj)
    
    shapeKeyKeyObj = sorted(shapeKeysFromObject(keyObj)[0], key=lambda shapeKey: shapeKey.name)
    shapeKeysFCurvesRef = shapeKeyFCurvesFromObject(refObj)
    
    if len(shapeKeyKeyObj) != 0 and len(shapeKeysFCurvesRef) != 0:
        shapeKeysDict = dict()
        
        # dict that puts the name of the shape keys and then the actual shapeKey to be keyed
        for shapeKey in shapeKeyKeyObj:
            # always will be inserting vals
            shapeKeysDict[shapeKey.name] = [shapeKey]
        
        # now get data from shapeKeysFCurves and add it's reference FCurve (to be evaled from)
        for shapeFCrv in shapeKeysFCurvesRef:
            name = shapeFCrv.data_path
            name = split_path(name)[0].replace("key_blocks", "")[2:-2]
            
            if name in shapeKeysDict: 
                shapeKeysDict[name].append(shapeFCrv)
            
        # now we have list of the object to Key and now the FCurve to eval from
        for key in shapeKeysDict:
            dictVal = shapeKeysDict[key]
            if len(dictVal) != 2: continue
            
            objToKey = dictVal[0]
            fCrv = dictVal[1]
            
            # keyframing it goes here
            fCrvEval = fCrv.evaluate(value)
            objToKey.value = fCrvEval
            objToKey.keyframe_insert(data_path=fCrv.data_path.split(".")[-1], frame=frame)
            

    if len(fCurves) != 0:
        for fCrv in fCurves:
            fCrvEval = fCrv.evaluate(value)
            
            bpy.ops.wm.context_set_value(data_path=f"scene.objects['{keyObj.name}'].{fCrv.data_path}[{fCrv.array_index}]", value=f"{fCrvEval}")
            keyObj.keyframe_insert(data_path=fCrv.data_path, index=fCrv.array_index, frame=frame)

def cleanKeyframes(obj, channels: Set={"all_channels"}):
    fCurves = FCurvesFromObject(obj)
    
    for fCurve in fCurves:
        if {fCurve.data_path, "all_channels"}.intersection(channels):
            obj.animation_data.action.fcurves.remove(fCurve)

def secToFrames(sec: float) -> float:
    bScene = bpy.context.scene
    fps = bScene.render.fps / bScene.render.fps_base
    
    return sec * fps

def framesToSec(frame: float) -> float:
    bScene = bpy.context.scene
    fps = bScene.render.fps / bScene.render.fps_base
    
    return frame / fps

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

def setInterpolationForLastKeyframe(obj: bpy.types.Object, interpolation: str):
    with suppress(AttributeError):
        if obj is not None and obj.animation_data is not None and obj.animation_data.action is not None:
            for fCrv in FCurvesFromObject(obj):
                fCrv.keyframe_points[-1].interpolation = interpolation

def deleteMarkers(name: str):
    scene = bpy.context.scene
    for marker in scene.timeline_markers:
        if name in marker.name:
            scene.timeline_markers.remove(marker)

# This method could be useful to calculate the distance between 2 objects (think: drumsticks?)
def distanceFromVectors(point1: Vector, point2: Vector) -> float: 
    """Calculate distance between two points.""" 
    
    return (point2 - point1).length
