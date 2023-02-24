from __future__ import annotations
from typing import Dict, List, Tuple, Optional, Union
from math import floor, ceil, radians
from mathutils import Vector, Euler
from dataclasses import dataclass
from contextlib import suppress
from itertools import zip_longest
from pprint import pprint
import bpy

from ..data_structures.midi import MIDITrack
from .. utils import convertNoteNumbers
from .. utils.loggerSetup import *
from .. data_structures import *
from .. utils.blender import *
from . algorithms import *

@dataclass
class Instrument:
    """base class for instruments that are played for notes"""
    collection: bpy.types.Collection
    midiTrack: MIDITrack
    noteToWpr: Dict[int, bpy.types.Object]

    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, override=False):
        self.collection = collection
        self.midiTrack = midiTrack
        self.noteToWpr = dict()
        self.override = override
        self.preAnimate()

        if not self.override:
            self.__post_init__()
    
    def __post_init__(self):
        # ensure objects of keyframed type have either note on or note off FCurve objects
        for obj in self.collection.all_objects:
            if obj.midi.anim_type == "keyframed" and obj.midi.note_on_curve is None and obj.midi.note_off_curve is None:
                raise ValueError("Animation type `keyframed` must have either a Note On Curve or a Note Off Curve!")
        
        self.createNoteToBlenderWpr()

    def getFCurves(self, obj: bpy.types.Object, noteType: str="note_on") -> List[Union[bpy.types.FCurve, ObjectShapeKey]]:
        assert noteType == "note_on" or noteType == "note_off", "Only types 'note_on' or 'note_off' are supported!"
        
        if obj.midi.anim_type != "keyframed": return ()
        
        if noteType == "note_on":
            objAnimObject = obj.midi.note_on_curve
        elif noteType == "note_off":
            objAnimObject = obj.midi.note_off_curve
        else:
            raise ValueError("Type needs to be 'note_on' or 'note_off'!")
        
        # if the object doesn't exist, just return None
        if not objAnimObject: return ()

        shapeKeysDict = {}
    
        # TODO theres probably a better way to make this work. Eventually I want ObjectShapeKeys to be an immutable class, using frozen dataclasses.
        # first need to get all of this reference object's shape key FCurves
        for fCrv in shapeKeyFCurvesFromObject(objAnimObject):
            if fCrv.data_path[-5:] == "value":  # we only want it if they have keyframed "value"
                # fCrv.data_path returns 'key_blocks["name"].value'.
                # 'key_blocks["' will never change and so will '"].value'.
                # chopping those off gives us just the name
                
                name = fCrv.data_path[12:-8]
                objKeys = ObjectShapeKey(name=name, referenceCurve=fCrv, targetKey=None)
                shapeKeysDict[name] = objKeys  # always 1 FCurve for 1 shape key

        # now get this object's shape keys so we can insert keyframes on them
        for shpKey in shapeKeysFromObject(obj)[0]:  #only want the shape keys, not the basis, see func for more info
            if shpKey.name in shapeKeysDict:
                shapeKeysDict[shpKey.name].targetKey = shpKey
        
        # delete unused shape keys (these are the keys that would be on the reference object)
        for key in shapeKeysDict.copy():
            val = shapeKeysDict[key]
            if val.targetKey == None: 
                del shapeKeysDict[key]
        
        out = []

        for fCrv in FCurvesFromObject(objAnimObject):
            out.append(fCrv)

        for key in shapeKeysDict:
            objShpKey = shapeKeysDict[key]
            out.append(objShpKey)
        
        return out

    def createNoteToBlenderWpr(self) -> None:
        allUsedNotes = self.midiTrack.allUsedNotes()

        for obj in self.collection.all_objects:
            if obj.midi.note_number is None or not obj.midi.note_number: raise ValueError(f"Object '{obj.name}' has no note number!")

            # make sure objects are not in target collection
            assert not any(item in set((obj.midi.note_on_curve, obj.midi.note_off_curve)) for item in set(self.collection.all_objects)), "Animation reference objects are in the target animation collection! Please move them out of the collection."

            wpr = BlenderWrapper(
                obj=obj, 
                noteNumbers=convertNoteNumbers(obj.midi.note_number), 
                noteOnCurves=self.getFCurves(obj=obj, noteType="note_on"), 
                noteOffCurves=self.getFCurves(obj=obj, noteType="note_off")
            )
            
            for noteNumber in wpr.noteNumbers:
                if noteNumber not in allUsedNotes:
                    print(f"WARNING: Object `{wpr.obj.name}` with MIDI note `{noteNumber}` does not exist in the MIDI track provided (MIDI track `{self.midiTrack.name}`)!")

                if noteNumber in self.noteToWpr:
                    self.noteToWpr[noteNumber].append(wpr)
                else:
                    self.noteToWpr[noteNumber] = [wpr]

    def preAnimate():
        """actions to take before the animation starts (cleaning keyframes, setting up objects, etc.)
        this method is called on class initalization.
        subclass should override this method
        """
        pass

    def animate(self):
        """actual animation. this method is only called once
        subclass should override this method
        """
        raise RuntimeError("subclass should override animate() method. See the docs on how to create custom instruments.")

class ProjectileInstrument(Instrument):
    _cacheInstance: Optional[CacheInstance]

    def __init__(self, midiTrack: MIDITrack, objectCollection: bpy.types.Collection, projectileCollection: bpy.types.Collection, referenceProjectile: bpy.types.Object):
        super().__init__(midiTrack, objectCollection)

        self._cacheInstance = None
        self.projectileCollection = projectileCollection
        self.referenceProjectile = referenceProjectile

    def preAnimate(self):
        # delete old objects
        objectCollection = self.collection

        cleanCollection(self.projectileCollection, self.referenceProjectile)

        # calculate number of needed projectiles & instance the blender objects
        maxNumOfProjectiles = maxSimultaneousObjects(self._objFrameRanges)

        projectiles = []

        for i in range(maxNumOfProjectiles):
            duplicate = self.referenceProjectile.copy()
            duplicate.name = f"projectile_{hex(id(objectCollection))}_{i}"
            
            # hide them
            showHideObj(duplicate, True, self._objFrameRanges[0].startFrame)

            self.projectileCollection.objects.link(duplicate)
            projectiles.append(duplicate)
        
        # create CacheInstance object
        self._cacheInstance = CacheInstance(projectiles)

class EvaluateInstrument(Instrument):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

    def preAnimate(self):
        bpy.context.scene.frame_set(-10000)
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)
    
    def animate(self):
        for note in self.midiTrack.notes:
            # lookup blender object
            if note.noteNumber in self.noteToWpr:
                wprs = self.noteToWpr[note.noteNumber]
            else: 
                continue
            
            # iterate over all "wrapped" Blender objects
            for wpr in wprs:
                obj = wpr.obj

                if obj.midi.anim_type == "keyframed":
                    # at this point, note On and note Off curves should be identical lengths, as cheked previously by validateFCurves() in the wpr init method
                    # so we can iterate over 1 set of FCurves, and get their datapaths
                    # but if there are no noteOnCurves but there are noteOffCurves, we will need to iterate over that instead 
                    for (fCrv, noteOffCurve) in zip_longest(wpr.noteOnCurves, wpr.noteOffCurves):
                        nextKeys = []

                        if not fCrv:
                            # find the keyframe lists for this particular FCurve
                            key = (fCrv.data_path, fCrv.array_index)
                        else:
                            # try using noteOffCurve if there is no noteOnCurve
                            key = (noteOffCurve.data_path, noteOffCurve.array_index)


                        if key not in wpr.keyframes.listOfKeys:
                            wpr.keyframes.listOfKeys[key] = []
                        
                        keyframes = wpr.keyframes.listOfKeys[key]
                        
                        if wpr.noteOnCurves:
                            if isinstance(fCrv, bpy.types.FCurve):
                                for keyframe in fCrv.keyframe_points:
                                    frame = keyframe.co[0] + secToFrames(note.timeOn) + wpr.obj.midi.note_on_anchor_pt
                                    if wpr.obj.midi.velocity_intensity != 0:
                                        value = keyframe.co[1] * (note.velocity / 127) * wpr.obj.midi.velocity_intensity
                                    else:
                                        value = keyframe.co[1]
                                    nextKeys.append(Keyframe(frame, value))
                            
                            elif isinstance(fCrv, ObjectShapeKey):
                                # shape keys handle differently
                                for keyframe in fCrv.referenceCurve.keyframe_points:
                                    frame = keyframe.co[0] + secToFrames(note.timeOn) + wpr.obj.midi.note_on_anchor_pt
                                    if wpr.obj.midi.velocity_intensity != 0:
                                        value = keyframe.co[1] * (note.velocity / 127) * wpr.obj.midi.velocity_intensity
                                    else:
                                        value = keyframe.co[1]
                                    nextKeys.append(Keyframe(frame, value))
                        
                        if wpr.noteOffCurves:
                            if isinstance(noteOffCurve, bpy.types.FCurve):
                                for keyframe in noteOffCurve.keyframe_points:
                                    frame = keyframe.co[0] + secToFrames(note.timeOff) + wpr.obj.midi.note_off_anchor_pt
                                    if wpr.obj.midi.velocity_intensity != 0:
                                        value = keyframe.co[1] * (note.velocity / 127) * wpr.obj.midi.velocity_intensity
                                    else:
                                        value = keyframe.co[1]
                                    nextKeys.append(Keyframe(frame, value))
                            
                            elif isinstance(noteOffCurve, ObjectShapeKey):
                                # shape keys handle differently
                                for keyframe in noteOffCurve.referenceCurve.keyframe_points:
                                    frame = keyframe.co[0] + secToFrames(note.timeOff) + wpr.obj.midi.note_off_anchor_pt
                                    if wpr.obj.midi.velocity_intensity != 0:
                                        value = keyframe.co[1] * (note.velocity / 127) * wpr.obj.midi.velocity_intensity
                                    else:
                                        value = keyframe.co[1]
                                    nextKeys.append(Keyframe(frame, value))


                        # take keyframes that are next and "add" them to the already insrted keyframes
                        if obj.midi.anim_overlap == "add":
                            addKeyframes(insertedKeys=keyframes, nextKeys=nextKeys)
                
                elif obj.midi.anim_type == "osc":
                    # for now, we're going to use a keyframed object to determine which channels to keyframe to
                    # this will eventually be replaced with a more permanent solution, like a UI element where you can add the different channels
                    # evaluate the parameters on the object and add them to list of nextKeys list
                    # using function genDampedOscKeyframes()
                    pass
                
                elif obj.midi.anim_type == "adsr":
                    # for now, we're going to use a keyframed object to determine which channels to keyframe to
                    # this will eventually be replaced with a more permanent solution, like a UI element where you can add the different channels
                    # evaluate the parameters on the object and add them to list of nextKeys list
                    # using function genADSRKeyframes() (not yet implemented)
                    pass

        
        # write keyframes after iterating over all notes
        for noteNumber in self.noteToWpr:
            for wpr in self.noteToWpr[noteNumber]:
                obj = wpr.obj

                if obj.midi.anim_type == "keyframed":
                    for noteOnCurve, noteOffCurve in zip_longest(wpr.noteOnCurves, wpr.noteOffCurves):
                        # make sure curve exists. if it doesn't this is probably a noteOff only object
                        if noteOnCurve is not None:
                            fCrv = noteOnCurve
                        else:
                            fCrv = noteOffCurve
                        
                        # if the object does not play anything (no MIDI notes read == no keyframes to write)
                        if (fCrv.data_path, fCrv.array_index) not in wpr.keyframes.listOfKeys: continue
                        keyframes = wpr.keyframes.listOfKeys[(fCrv.data_path, fCrv.array_index)]

                        if isinstance(fCrv, bpy.types.FCurve):
                            for keyframe in sorted(keyframes, key=lambda x: x.frame):
                                # set value
                                if fCrv.data_path[:2] == '["' and fCrv.data_path[-2:] == '"]':
                                    # custom prop
                                    exec(f"bpy.data.objects['{obj.name}']{fCrv.data_path} = {keyframe.value}")
                                    obj.keyframe_insert(data_path=fCrv.data_path, frame=keyframe.frame)
                                else:
                                    # every other datapath
                                    # add initial values
                                    if fCrv.data_path == "location":
                                        value = keyframe.value + wpr.initalLoc[fCrv.array_index]
                                    elif fCrv.data_path == "rotation_euler":
                                        value = keyframe.value + wpr.initalRot[fCrv.array_index]
                                    elif fCrv.data_path == "scale":
                                        value = keyframe.value + wpr.initalScl[fCrv.array_index]
                                    else:
                                        value = keyframe.value
                                    
                                    exec(f"bpy.data.objects['{obj.name}'].{fCrv.data_path}[{fCrv.array_index}] = {value}")
                                    obj.keyframe_insert(data_path=fCrv.data_path, index=fCrv.array_index, frame=keyframe.frame)
                        elif isinstance(fCrv, ObjectShapeKey):
                            for keyframe in sorted(keyframes, key=lambda x: x.frame):
                                fCrv.targetKey.value = keyframe.value
                                fCrv.targetKey.keyframe_insert(data_path="value", frame=keyframe.frame)
                else:
                    raise RuntimeError(f"ERROR: Type {obj.midi.anim_type} for object {obj.name} not supported yet.")
