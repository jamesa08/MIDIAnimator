from __future__ import annotations
from typing import Dict, List, Tuple, Optional, Union
from math import floor, ceil, radians
from mathutils import Vector, Euler
from dataclasses import dataclass
from contextlib import suppress
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

        if not self.override:
            self.__post_init__()
    
    def __post_init__(self):
        # ensure objects of keyframed type have either note on or note off FCurve objects
        for obj in self.collection.all_objects:
            if obj.midi.anim_type == "keyframed" and obj.midi.note_on_curve is None and obj.midi.note_off_curve is None:
                raise ValueError("Animation type `keyframed` must have either a Note On Curve or a Note Off Curve!")
        
        self.createNoteToBlenderWpr()
    
    def getFCurves(self, obj: bpy.types.Object, noteType: str="note_on") -> ObjectFCurvesNew:
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

        # bpy.context.scene.frame_set(-10000)
        
        fCurves: List[bpy.types.FCurve] = []

        location = []
        rotation = []
        shapeKeysDict = {}
        customProperties = []
        
        for fCrv in FCurvesFromObject(objAnimObject):
            dataPath = fCrv.data_path
            fCurves.append(fCrv)

            if dataPath == "location":
                location.append(fCrv)
            elif dataPath == "rotation_euler":
                rotation.append(fCrv)
            # custom property
            elif dataPath[:2] == '["' and dataPath[-2:] == '"]':
                getType = eval(f"type(bpy.context.scene.objects['{objAnimObject.name}']{dataPath})")
                assert getType == float or getType == int, "Please create type `int` or type `float` custom properties"
                customProperties.append(fCrv)
        
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
        for key in shapeKeysDict.copy():  # NOTE: .keys() did not work, same error (dict change size during iteration   )
            val = shapeKeysDict[key]
            if val.targetKey == None: 
                del shapeKeysDict[key]
        
        out = []
        # FIXME temp (?)        
        for fCrv in fCurves:
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
                    print(f"WARNING: Object `{wpr.obj.name}` (MIDI note {noteNumber}) does not exist in the MIDI track (MIDI track {self.midiTrack.name}!")

                if noteNumber in self.noteToWpr:
                    self.noteToWpr[noteNumber].append(wpr)
                else:
                    self.noteToWpr[noteNumber] = [wpr]

    def cleanup(self):
        self.noteToWpr = dict()

    def animate(self):
        for note in self.midiTrack.notes:
            # lookup blender object
            if note.noteNumber in self.noteToWpr:
                wprs = self.noteToWpr[note.noteNumber]
            else: 
                continue

            # iterate over all objects
            for wpr in wprs:
                obj = wpr.obj

                if obj.midi.anim_type == "keyframed":
                    # note On Curves
                    for fCrv in wpr.noteOnCurves:
                        nextKeys = []

                        # find the keyframe lists for this particular FCurve
                        if fCrv not in wpr.noteOnKeyframes.listOfKeys:
                            wpr.noteOnKeyframes.listOfKeys[fCrv] = []
                        
                        keyframesOn = wpr.noteOnKeyframes.listOfKeys[fCrv]
                        
                        if isinstance(fCrv, bpy.types.FCurve):
                            for keyframe in fCrv.keyframe_points:
                                frame = keyframe.co[0] + secToFrames(note.timeOn) + wpr.obj.midi.note_on_anchor_pt
                                value = keyframe.co[1]
                                nextKeys.append(Keyframe(frame, value))
                            
                            # take keyframes that are next and "add" them to the already insrted keyframes
                            if obj.midi.anim_overlap == "add":
                                addKeyframes(insertedKeys=keyframesOn, nextKeys=nextKeys)
                        
                        elif isinstance(fCrv, ObjectShapeKey):
                            # shape keys handle differently
                            for keyframe in fCrv.referenceCurve.keyframe_points:
                                frame = keyframe.co[0] + secToFrames(note.timeOn) + wpr.obj.midi.note_on_anchor_pt
                                value = keyframe.co[1]
                                nextKeys.append(Keyframe(frame, value))
                            
                            # take keyframes that are next and "add" them to the already insrted keyframes
                            if obj.midi.anim_overlap == "add":
                                addKeyframes(insertedKeys=keyframesOn, nextKeys=nextKeys)
                            
                    # note Off Curves
                    for fCrv in wpr.noteOffCurves:
                        nextKeys = []

                        # find the keyframe lists for this particular FCurve
                        if fCrv not in wpr.noteOffKeyframes.listOfKeys:
                            wpr.noteOffKeyframes.listOfKeys[fCrv] = []
                        
                        keyframesOff = wpr.noteOffKeyframes.listOfKeys[fCrv]
                        
                        if isinstance(fCrv, bpy.types.FCurve):
                            for keyframe in fCrv.keyframe_points:
                                frame = keyframe.co[0] + secToFrames(note.timeOn) + wpr.obj.midi.note_off_anchor_pt
                                value = keyframe.co[1]
                                nextKeys.append(Keyframe(frame, value))
                            
                            # take keyframes that are next and "add" them to the already insrted keyframes
                            if obj.midi.anim_overlap == "add":
                                addKeyframes(insertedKeys=keyframesOff, nextKeys=nextKeys)
                        
                        elif isinstance(fCrv, ObjectShapeKey):
                            for keyframe in fCrv.referenceCurve.keyframe_points:
                                frame = keyframe.co[0] + secToFrames(note.timeOn) + wpr.obj.midi.note_off_anchor_pt
                                value = keyframe.co[1]
                                nextKeys.append(Keyframe(frame, value))
                            
                            # take keyframes that are next and "add" them to the already insrted keyframes
                            if obj.midi.anim_overlap == "add":
                                addKeyframes(insertedKeys=keyframesOff, nextKeys=nextKeys)
                    
                elif obj.midi.anim_type == "osc":
                    # for now, we're going to use a keyframed object to determine which channels to keyframe to
                    # this will eventually be replaced with a more permanent solution, like a UI element where you can add the different channels
                    # evaluate the parameters on the object and add them to list of nextKeys list
                    # using function genDampedOscKeyframes()
                    pass
                
                elif obj.midi.anim_type == "adsr":
                    # evaluate the parameters on the object and add them to list of nextKeys list
                    # using function genADSRKeyframes() (not yet implemented)
                    pass

        
        # write keyframes after iterating over all notes
        for wpr in wprs:
            obj = wpr.obj
            for fCrv in wpr.noteOnCurves:
                keyframesOn = wpr.noteOnKeyframes.listOfKeys[fCrv]
                
                # FIXME this isnt correct. FCurves from both noteOn and noteOff lists are different. Need a new method to find matching FCurves
                if fCrv in wpr.noteOffKeyframes.listOfKeys:
                    keyframesOff = wpr.noteOffKeyframes.listOfKeys[fCrv]
                else:
                    keyframesOff = []
                
                # keyframesOff = wpr.noteOffKeyframes.listOfKeys[list(wpr.noteOffKeyframes.listOfKeys)[0]]
                
                print("x, y")
                for keyframe in sorted(keyframesOn + keyframesOff, key=lambda x: x.frame):
                    print(f"{keyframe.frame}, {keyframe.value}")

                if isinstance(fCrv, bpy.types.FCurve):
                    for keyframe in sorted(keyframesOn + keyframesOff, key=lambda x: x.frame):
                        # set value
                        if fCrv.data_path[:2] == '["' and fCrv.data_path[-2:] == '"]':
                            # custom prop
                            exec(f"bpy.data.objects['{obj.name}']{fCrv.data_path} = {keyframe.value}")
                            obj.keyframe_insert(data_path=fCrv.data_path, frame=keyframe.frame)
                        else:
                            exec(f"bpy.data.objects['{obj.name}'].{fCrv.data_path}[{fCrv.array_index}] = {keyframe.value}")
                            obj.keyframe_insert(data_path=fCrv.data_path, index=fCrv.array_index, frame=keyframe.frame)
                elif isinstance(fCrv, ObjectShapeKey):
                    for keyframe in sorted(keyframesOn + keyframesOff, key=lambda x: x.frame):
                        fCrv.targetKey.value = keyframe.value
                        fCrv.targetKey.keyframe_insert(data_path="value", frame=keyframe.frame)

            # for fCrv in wpr.noteOffCurves:
            #     keyframesOff = wpr.noteOffKeyframes.listOfKeys[fCrv]

            #     if isinstance(fCrv, bpy.types.FCurve):
            #         for keyframe in keyframesOff:
            #             # set value
            #             if fCrv.data_path[:2] == '["' and fCrv.data_path[-2:] == '"]':
            #                 # custom prop
            #                 exec(f"bpy.data.objects['{obj.name}']{fCrv.data_path} = {keyframe.value}")
            #                 obj.keyframe_insert(data_path=fCrv.data_path, frame=keyframe.frame)
            #             else:
            #                 exec(f"bpy.data.objects['{obj.name}'].{fCrv.data_path}[{fCrv.array_index}] = {keyframe.value}")
            #                 obj.keyframe_insert(data_path=fCrv.data_path, index=fCrv.array_index, frame=keyframe.frame)
            #     elif isinstance(fCrv, ObjectShapeKey):
            #         for keyframe in keyframesOff:
            #             fCrv.targetKey.value = keyframe.value
            #             fCrv.targetKey.keyframe_insert(data_path="value", frame=keyframe.frame)


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

        self.preAnimate()

    def preAnimate(self):
        bpy.context.scene.frame_set(-10000)
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)
        bpy.context.scene.frame_set(0)