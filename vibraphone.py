from MIDIAnimator.src.animation import *
from MIDIAnimator.utils.blender import *
from MIDIAnimator.utils import *
from typing import Callable, List, Tuple, Dict, Optional, Union
from MIDIAnimator.data_structures.midi import MIDIFile
import bpy

class FCurveProcessorNew:
    obj: bpy.types.Object
    locationObject: Optional[bpy.types.Object]
    fCurves: ObjectFCurves
    location: Optional[Vector]
    rotation: Optional[Euler]
    
    # key= custom property name, value=int or float (the val to be keyframed)
    material: Dict[str, Union[int, float]]
    
    # key= the "to keyframe" object's shape key's name, value= a float (the val to be keyframed)
    shapeKeys: Dict[str, float]

    def __init__(self, obj: bpy.types.Object, fCurves: ObjectFCurves, locationObject: Optional[bpy.types.Object] = None):
        self.obj = obj
        self.fCurves = fCurves
        self.locationObject = locationObject
        # when None no keyframe of that type
        self.location = Vector()
        self.rotation = Euler()
        self.material = {}
        self.shapeKeys = {}
        self.vibraphone = True


    def applyFCurve(self, delta: int):
        if self.vibraphone and len(self.fCurves.location) == 0:
            delta *= (0.27 + int(self.obj.note_number) / 79) ** 1.3

        if len(self.fCurves.location) != 0:
            if self.locationObject is None:
                location = self.obj.location.copy()
            else:
                location = self.locationObject.location.copy()
        
            if not self.vibraphone:
                for fCurve in self.fCurves.location:
                    i = fCurve.array_index
                    val = fCurve.evaluate(delta)
                    location[i] = val
            else:
                radiusX = 0
                angleY = 0
                heightZ = 0

                # get object's index value for angleY
                # we'll need a better way of doing this, this could be taxing (?)
                objIndex = 0
                with suppress(ValueError):
                    objIndex = int(self.locationObject.name[-3:])
                    angleY = radians(((objIndex*360)/40) - -314.5)  # 40 is the amount of objects

                for fCurve in self.fCurves.location:
                    i = fCurve.array_index
                    if i == 0:  # x val
                        radiusX = fCurve.evaluate(delta)
                
                    if i == 2:  # z val
                        heightZ = fCurve.evaluate(delta)

                # set the values on internal location
                x, y = rotateAroundCircle(radiusX, angleY)
                location = Vector((y, x, heightZ))
            
            # set the values on internal location
            self.location = location

        if len(self.fCurves.rotation) != 0:
            # if self.rotation is None:
            #     rotation = self.obj.rotation_euler.copy()
            # else:
            #     rotation = self.rotation
            if self.rotation is None:
                rotation = Euler()
            else:
                rotation = self.rotation
            
            for fCurve in self.fCurves.rotation:
                i = fCurve.array_index
                val = fCurve.evaluate(delta)
                rotation[i] += val

            # set the values on internal rotation
            self.rotation = rotation

        if len(self.fCurves.material) != 0:
            for fCurve in self.fCurves.material:
                val = fCurve.evaluate(delta)
                if fCurve.data_path in self.material:
                    self.material[fCurve.data_path] += val
                else:
                    self.material[fCurve.data_path] = val

        if len(self.fCurves.shapeKeysDict) != 0:
            for shapeName in self.fCurves.shapeKeysDict:
                val = self.fCurves.shapeKeysDict[shapeName][0].evaluate(delta)  # we want to get only the FCurve from the dict (index 0 is the FCurve)

                if shapeName in self.shapeKeys:
                    self.shapeKeys[shapeName] += val
                else:
                    self.shapeKeys[shapeName] = val

    def insertKeyFrames(self, frame: int):
        if self.location is not None:
            # summed = sum([processor.location for processor in self.linkedProcessors])  future update
            self.obj.location = npAdd(self.location, self.fCurves.origLoc)
            self.obj.keyframe_insert(data_path="location", frame=frame)
            setKeyframeInterpolation(self.obj, "BEZIER")
        
        if self.rotation is not None:
            self.obj.rotation_euler = npAdd(self.rotation, self.fCurves.origRot)
            self.obj.keyframe_insert(data_path="rotation_euler", frame=frame)
        
        if self.material is not None:
            for data_path in self.material:
                val = self.material[data_path]
                exec(f"bpy.context.scene.objects['{self.obj.name}']{data_path} = {val}")
                self.obj.keyframe_insert(data_path=data_path, frame=frame)

        if len(self.shapeKeys) != 0:
            for shapeName in self.fCurves.shapeKeysDict:
                shapeKey = self.fCurves.shapeKeysDict[shapeName][1]  # index 1 is the shape Key to keyframe

                shapeKey.value = self.shapeKeys[shapeName]
                shapeKey.keyframe_insert(data_path="value", frame=frame)

    def __repr__(self) -> str:
        return f"{self.obj} {self.locationObject} {self.fCurves} {self.location} {self.rotation} {self.material} {self.shapeKeys} {self.linkedProcessors}"

class VibraphoneProjectileInstrument(Instrument):
    _cacheInstance: Optional[CacheInstance]

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # post init things here
        self._cacheInstance = None

    def preAnimate(self):
        # delete old objects
        objectCollection = self.collection

        assert objectCollection.projectile_collection is not None, "Please define a Projectile collection for type Projectile."
        assert objectCollection.reference_projectile is not None, "Please define a reference object to be duplicated."
        cleanCollection(objectCollection.projectile_collection, objectCollection.reference_projectile)

        # calculate number of needed projectiles & instance the blender objects using bpy
        maxNumOfProjectiles = maxSimultaneousObjects(self._objFrameRanges)

        projectiles = []

        for i in range(maxNumOfProjectiles):
            duplicate = objectCollection.reference_projectile.copy()
            duplicate.name = f"projectile_{hex(id(self.midiTrack))}_{i}"
            
            # hide them
            showHideObj(duplicate, True, self._objFrameRanges[0].startFrame)

            objectCollection.projectile_collection.objects.link(duplicate)
            projectiles.append(duplicate)
        
        # create CacheInstance object
        self._cacheInstance = CacheInstance(projectiles)

    def animate(self, frame: int, offset: int):
        # each cached object needs a separate FCurveProcessor since the same note can be "in progress" more than
        # once for a given frame
        cacheObjectProcessors = {}
        # for this note, iterate over all frame ranges for the note being played with objects still moving
        for noteNumber in self._activeNoteDict:
            # if finished playing all instances of this note, skip to next note
            if len(self._activeNoteDict[noteNumber]) == 0:
                continue

            # set of FCurveProcessor in case more than one cached object currently in-progress for the same note
            processorSet = set()
            for bObj in self.noteToBlenderObject[noteNumber]:
                processor = FCurveProcessorNew(bObj.obj, bObj.fCurves)

                for frameInfo in self._activeNoteDict[noteNumber]:
                    if frameInfo.bObj.obj != bObj.obj: continue
                    processorSet.add(processor)

                    # check if we are animating a cached object for this
                    cachedObj = frameInfo.cachedObj
                    if cachedObj is not None:
                        # if not the first time we are using this cachedObject, get its FCurveProcessor
                        if cachedObj in cacheObjectProcessors:
                            processor = cacheObjectProcessors[cachedObj]
                        # this is the first time this cachedObject is used so need to create its FCurveProcessor
                        else:
                            # create a processor and include in set for keyframing
                            processor = FCurveProcessorNew(cachedObj, frameInfo.bObj.fCurves, frameInfo.bObj.obj)
                            processorSet.add(processor)
                            cacheObjectProcessors[cachedObj] = processor

                    # accumulate the FCurve results for this object
                    # need this step for cymbals since we need to add the FCurves for each instance of the note
                    objStartFrame = frameInfo.startFrame
                    delta = frame - objStartFrame
                    # assert frameInfo.bObj.obj == bObj.obj, f"{frameInfo.bObj.obj=} and {bObj.obj=} are not the same!"
                    processor.applyFCurve(delta)

            # for each object (could be multiple cached objects) insert the key frame
            for p in processorSet:
                p.insertKeyFrames(frame + offset)


class VibraphoneInstrument(Instrument):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

        self.preAnimate()

    def preAnimate(self):
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)

    def animate(self, frame: int, offset: int):
        # each cached object needs a separate FCurveProcessor since the same note can be "in progress" more than
        # once for a given frame
        cacheObjectProcessors = {}
        # for this note, iterate over all frame ranges for the note being played with objects still moving
        for noteNumber in self._activeNoteDict:
            # if finished playing all instances of this note, skip to next note
            # TODO see if this is what is breaking the last note? I think it is
            if len(self._activeNoteDict[noteNumber]) == 0:
                continue

            # for non projectiles, this will be the same object each through the inner for loop
            
            frameInfo = self._activeNoteDict[noteNumber][0]
            bObj = frameInfo.bObj
            obj = bObj.obj

            # set of FCurveProcessor in case more than one cached object currently in-progress for the same note
            processorSet = set()
            for bObj in self.noteToBlenderObject[noteNumber]:
                # get the ObjectFCurves for this note
                objFCurve = bObj.fCurves
                # make a processor for it
                processor = FCurveProcessorNew(obj, objFCurve)
                # keep track of all objects that need keyframed
                processorSet.add(processor)
            
                for frameInfo in self._activeNoteDict[noteNumber]:
                    obj = frameInfo.bObj.obj
                    # check if we are animating a cached object for this
                    cachedObj = frameInfo.cachedObj
                    if cachedObj is not None:
                        # if not the first time we are using this cachedObject, get its FCurveProcessor
                        if cachedObj in cacheObjectProcessors:
                            processor = cacheObjectProcessors[cachedObj]
                        # this is the first time this cachedObject is used so need to create its FCurveProcessor
                        else:
                            # create a processor and include in set for keyframing
                            processor = FCurveProcessorNew(cachedObj, frameInfo.bObj.fCurves, obj)
                            processorSet.add(processor)
                            cacheObjectProcessors[cachedObj] = processor

                    # accumulate the FCurve results for this object
                    # need this step for cymbals since we need to add the FCurves for each instance of the note
                    objStartFrame = frameInfo.startFrame
                    delta = frame - objStartFrame
                    processor.applyFCurve(delta)

            # for each object (could be multiple cached objects) insert the key frame
            for p in processorSet:
                p.insertKeyFrames(frame + offset)


# file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/pipedream3_8_18_21_1.mid")
file = MIDIFile("/Users/james/github/MIDIFiles/testMidi/pd1_vibe.mid")

tracks = file.getMIDITracks()
vibeTrack = file.findTrack("Vibraphone")
# vibraphoneNotes = [36,38,39,40,41,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,65,66,67,68,69,70,71,72,73,74,75,76,77,79]
vibraphoneNotes = [36,38,39,40,41,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,69,70,71,72,73,74,75,76,77,79]

scene = bpy.context.scene

# funnels
scene.quick_instrument_type = "projectile"
scene["note_number_list"] = str(vibraphoneNotes)
scene["quick_obj_col"] = bpy.data.collections['FunnelEmpties']
scene["quick_obj_curve"] = bpy.context.scene.objects['ANIM_VibeBall']
scene["quick_use_sorted"] = True
scene["quick_note_hit_time"] = 28
bpy.ops.scene.quick_add_props()

# strings
scene.quick_instrument_type = "string"
scene["quick_obj_col"] = bpy.data.collections['PP1']
scene["quick_obj_curve"] = bpy.context.scene.objects['ANIM_PP1']
scene["quick_use_sorted"] = True
scene["quick_note_hit_time"] = 0
bpy.ops.scene.quick_add_props()


scene.quick_instrument_type = "string"
scene["quick_obj_col"] = bpy.data.collections['PP2']
scene["quick_obj_curve"] = bpy.context.scene.objects['ANIM_PP2']
scene["quick_note_hit_time"] = 0
bpy.ops.scene.quick_add_props()

# scene.quick_instrument_type = "string"
# scene["quick_obj_col"] = bpy.data.collections['PP3']
# scene["quick_obj_curve"] = bpy.context.scene.objects['ANIM_PP3']
# scene["quick_note_hit_time"] = 0
# bpy.ops.scene.quick_add_props()

scene.quick_instrument_type = "string"
scene["quick_obj_col"] = bpy.data.collections['Keys']
scene["quick_obj_curve"] = bpy.context.scene.objects['ANIM_VibeKey']
scene["quick_note_hit_time"] = 0
bpy.ops.scene.quick_add_props()

animator = MIDIAnimatorNode()
animator.addInstrument(midiTrack=vibeTrack, objectCollection=bpy.data.collections['FunnelEmpties'], custom=VibraphoneProjectileInstrument)
animator.addInstrument(midiTrack=vibeTrack, objectCollection=bpy.data.collections['PP1'], custom=VibraphoneInstrument)
animator.addInstrument(midiTrack=vibeTrack, objectCollection=bpy.data.collections['PP2'], custom=VibraphoneInstrument)
# animator.addInstrument(midiTrack=vibeTrack, objectCollection=bpy.data.collections['PP3'], custom=VibraphoneInstrument)
animator.addInstrument(midiTrack=vibeTrack, objectCollection=bpy.data.collections['Keys'], custom=VibraphoneInstrument)
animator.animate()