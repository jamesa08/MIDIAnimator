from __future__ import annotations

from typing import Dict, List, Union, Type
from itertools import zip_longest
from dataclasses import dataclass
from math import radians, degrees
from enum import Enum
import bpy

from .. data_structures.midi import MIDITrack
from .. utils import convertNoteNumbers
from .. utils import animateAlongTwoPoints
from .. utils import mapRangeLinear as mLin, mapRangeLog as mLog, mapRangeExp as mExp, mapRangeArcSin as mASin, mapRangePara as mPara, mapRangeRoot as mRoot, mapRangeSin as mSin
from .. utils.logger import logger
from .. data_structures import *
from .. utils.blender import *
from . algorithms import *

class Instrument:
    """base class for MIDI instruments. These will handle all pre-animation and animation techniques."""
    collection: bpy.types.Collection
    midiTrack: MIDITrack

    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection):
        """Base class for MIDI instruments. These will handle all pre-animation and animation techniques.
        You should not instance this class by itself. This class should be inherited.

        :param MIDITrack midiTrack: the MIDITrack object to animate from
        :param bpy.types.Collection collection: the `bpy.types.Collection` of Blender objects to apply keyframes to
        """
        self.collection = collection
        self.midiTrack = midiTrack
    
    @staticmethod
    def drawInstrument(context: bpy.types.Context, col: bpy.types.UILayout, blCol: bpy.types.Collection,):
        """draws the UI for the instrument view
        subclass should override this method, this method will not do anything if called"""
        pass
    
    @staticmethod
    def drawObject(context: bpy.types.Context, col: bpy.types.UILayout, blObj: bpy.types.Object):
        """draws the UI for the object view
        subclass should override this method, this method will not do anything if called
        """
        pass

    @staticmethod
    def properties():
        """register properties for this instrument
        subclass should override this method, this method will not do anything if called
        """
        pass

    @staticmethod
    def getFCurves(animationObject: bpy.types.Object, referenceObject: bpy.types.Object) -> List[Union[bpy.types.FCurve, ObjectShapeKey]]:
        """gets the FCurves on a Blender Object.

        :param bpy.types.Object animationObject: the object to keyframe
        :param bpy.types.Object referenceObject: the object that holds the reference FCurves
        :return List[Union[bpy.types.FCurve, ObjectShapeKey]]: returns a list of either `bpy.types.FCurve`s and/or an `ObjectShapeKey`.
        """
        
        # if the object doesn't exist, just return None
        if not referenceObject: return ()

        shapeKeysDict = {}
    
        # TODO theres probably a better way to make this work. Eventually I want ObjectShapeKeys to be an immutable class, using frozen dataclasses.
        # first need to get all of this reference object's shape key FCurves
        for fCrv in shapeKeyFCurvesFromObject(referenceObject):
            if fCrv.data_path[-5:] == "value":  # we only want it if they have keyframed "value"
                # fCrv.data_path returns 'key_blocks["name"].value'.
                # 'key_blocks["' will never change and so will '"].value'.
                # chopping those off gives us just the name
                
                name = fCrv.data_path[12:-8]
                objKeys = ObjectShapeKey(name=name, referenceCurve=fCrv, targetKey=None)
                shapeKeysDict[name] = objKeys  # always 1 FCurve for 1 shape key

        # now get this object's shape keys so we can insert keyframes on them
        for shpKey in shapeKeysFromObject(animationObject)[0]:  #only want the shape keys, not the basis, see func for more info
            if shpKey.name in shapeKeysDict:
                shapeKeysDict[shpKey.name].targetKey = shpKey
        
        # delete unused shape keys (these are the keys that would be on the reference object)
        for key in shapeKeysDict.copy():
            val = shapeKeysDict[key]
            if val.targetKey == None: 
                del shapeKeysDict[key]
        
        out = []

        for fCrv in FCurvesFromObject(referenceObject):
            out.append(fCrv)

        for key in shapeKeysDict:
            objShpKey = shapeKeysDict[key]
            out.append(objShpKey)
        
        return out

    def preAnimate(self):
        """actions to take before the animation starts (cleaning keyframes, setting up objects, etc.)
        this method is called on class initalization.
        
        subclass should override this method, this method will not do anything if called
        """
        pass

    def animate(self):
        """this will keyframe the objects in `self.objectCollection`. this method is only called once
        
        subclass should override this method
        """
        raise RuntimeError("subclass should override animate() method. See the docs on how to create custom instruments.")


class ProjectileInstrument(Instrument):
    noteToWpr: Dict[int, bpy.types.Object]
    _cache: CacheInstance

    def __init__(self, midiTrack: MIDITrack, objectCollection: bpy.types.Collection):
        """Pre-defined animation code that animates a projectile launching from a funnel.
        :param MIDITrack midiTrack: the MIDITrack object to animate from
        :param bpy.types.Collection objectCollection: the `bpy.types.Collection` of Blender objects (funnels) to apply keyframes to. These are the funnel objects (starting position for the projectiles).
        """
        super().__init__(midiTrack, objectCollection)

        self.projectileCollection = objectCollection.midi.projectile_collection
        self.referenceProjectile = objectCollection.midi.reference_projectile
        
        assert self.projectileCollection is not None, "Please pass in a projectile collection!"
        assert self.referenceProjectile is not None, "Please pass in a reference projectile!"

        assert isinstance(self.projectileCollection, bpy.types.Collection), "Please pass in a bpy.types.Collection for property 'projectile_collection'."
        assert isinstance(self.referenceProjectile, bpy.types.Object), "Please pass in a bpy.types.Object for property 'reference_projectile'."
        
        # clean objects
        self.preAnimate()
        
        self._cache = CacheInstance()
        self.noteToWpr = dict()
        self.createNoteToObjectWpr()

    @staticmethod
    def drawInstrument(context: bpy.types.Context, col: bpy.types.UILayout, blCol: bpy.types.Collection):
        """draws the UI for the instrument view"""
        col.prop(blCol.midi, "projectile_collection")
        col.prop(blCol.midi, "reference_projectile")
        col.prop(blCol.midi, "use_initial_location")
        col.prop(blCol.midi, "use_circle_loc_map")
        if blCol.midi.use_circle_loc_map:
            col.prop(blCol.midi, "circle_loc_offset", text="Offset")
            col.prop(blCol.midi, "loc_collection", text="Location Collection")
    
    @staticmethod
    def drawObject(context: bpy.types.Context, col: bpy.types.UILayout, blObj: bpy.types.Object):
        """draws the UI for the object view"""
        col.prop(blObj.midi, "anim_curve", text="Projectile Curve")
        col.prop(blObj.midi, "hit_time")

    @staticmethod
    def properties():
        """create properties for the instrument"""
        MIDIAnimatorCollectionProperties.projectile_collection = bpy.props.PointerProperty(
            name="Projectile Collection",
            description="the `bpy.types.Collection` to store the projectiles",
            type=bpy.types.Collection,
            options=set()
        )
        MIDIAnimatorCollectionProperties.reference_projectile = bpy.props.PointerProperty(
            name="Reference Projectile",
            description="the `bpy.types.Object` to clone the projectile to. This object will not be animated, instead it will be copied.",
            type=bpy.types.Object,
            options=set()
        )
        MIDIAnimatorCollectionProperties.use_initial_location = bpy.props.BoolProperty(
            name="Use Inital Location",
            description="Use the inital location of the funnel object as the starting location for the projectile",
            default=True,
            options=set()
        )
        MIDIAnimatorCollectionProperties.use_circle_loc_map = bpy.props.BoolProperty(
            name="Use Angle Based Location",
            description="If all of the funnels are at a central point and you want the projectiles to radiate out (similar to a fountian), use this.",
            default=False,
            options=set()
        )
        MIDIAnimatorCollectionProperties.circle_loc_offset = bpy.props.FloatProperty(
            name="Angle Offset",
            description="Offset for the circle location mapping, in degrees.",
            default=0.0,
            subtype="ANGLE",
            options=set()
        )
        MIDIAnimatorCollectionProperties.loc_collection = bpy.props.PointerProperty(
            name="Location Collection",
            description="the `bpy.types.Collection` to locate the angle of the projectiles",
            type=bpy.types.Collection,
            options=set()
        )


        MIDIAnimatorObjectProperties.hit_time = bpy.props.IntProperty(
            name="Hit Time",
            description="Where the ball hits an object, this will get offset the animation, - for eariler, + for later",
            default=0,
            options=set()
        )
        MIDIAnimatorObjectProperties.anim_curve = bpy.props.PointerProperty(
            name="Projectile Animation Curve", 
            description="The projectile curve with defined keyframes to be read in. This is the curve that will be used to animate the projectiles.",
            type=bpy.types.Object,
            options=set()
        )

    @staticmethod
    def getFCurves(animationObject: bpy.types.Object, referenceObject: bpy.types.Object) -> List[Union[bpy.types.FCurve, ObjectShapeKey]]:
        """gets the FCurves on a Blender Object.

        :param bpy.types.Object animationObject: the object to keyframe
        :param bpy.types.Object referenceObject: the object that holds the reference FCurves
        :return List[Union[bpy.types.FCurve, ObjectShapeKey]]: returns a list of either `bpy.types.FCurve`s and/or an `ObjectShapeKey`.
        """
        
        # if the object doesn't exist, just return None
        if not referenceObject: return ()

        shapeKeysDict = {}

        # TODO theres probably a better way to make this work. Eventually I want ObjectShapeKeys to be an immutable class, using frozen dataclasses.
        # first need to get all of this reference object's shape key FCurves
        for fCrv in shapeKeyFCurvesFromObject(referenceObject):
            if fCrv.data_path[-5:] == "value":  # we only want it if they have keyframed "value"
                # fCrv.data_path returns 'key_blocks["name"].value'.
                # 'key_blocks["' will never change and so will '"].value'.
                # chopping those off gives us just the name
                
                name = fCrv.data_path[12:-8]
                objKeys = ObjectShapeKey(name=name, referenceCurve=fCrv, targetKey=None)
                shapeKeysDict[name] = objKeys  # always 1 FCurve for 1 shape key

        # now get this object's shape keys so we can insert keyframes on them
        for shpKey in shapeKeysFromObject(animationObject)[0]:  #only want the shape keys, not the basis, see func for more info
            if shpKey.name in shapeKeysDict:
                shapeKeysDict[shpKey.name].targetKey = shpKey
        
        # delete unused shape keys (these are the keys that would be on the reference object)
        for key in shapeKeysDict.copy():
            val = shapeKeysDict[key]
            if val.targetKey == None: 
                del shapeKeysDict[key]
        
        out = []
        loc = [False]*3

        for fCrv in FCurvesFromObject(referenceObject):
            if fCrv.data_path == "location":
                loc[fCrv.array_index] = True
            
            out.append(fCrv)
        
        # needed to add these in manually because they don't exist on the reference object
        # that way when a projectile gets keyframed, it will be at the correct location
        for i, bool in enumerate(loc):
            if not bool:
                out.append(DummyFCurve(keyframe_points=(Keyframe(frame=0, value=referenceObject.location[i]),), array_index=i, data_path="location"))


        for key in shapeKeysDict:
            objShpKey = shapeKeysDict[key]
            out.append(objShpKey)
        
        return out

    def createNoteToObjectWpr(self) -> None:
        """takes the MIDI file and builds a dictionary with the key being note number, and the value being a ObjectWrapper (which gets its FCurves)

        :raises ValueError: if there is no note number on the object
        """
        allUsedNotes = self.midiTrack.allUsedNotes()

        for obj in self.collection.all_objects:
            if obj.midi.note_number is None or not obj.midi.note_number: raise ValueError(f"Object '{obj.name}' has no note number!")

            # make sure objects are not in target collection
            assert not any(item == obj.midi.anim_curve for item in set(self.collection.all_objects)), "Animation reference objects are in the target animation collection! Please move them out of the collection."

            wpr = ObjectWrapper(
                obj=obj, 
                noteNumbers=convertNoteNumbers(obj.midi.note_number), 
                noteOnCurves=ProjectileInstrument.getFCurves(animationObject=obj, referenceObject=obj.midi.anim_curve),
                noteOffCurves=()
            )
            
            for noteNumber in wpr.noteNumbers:
                if noteNumber not in allUsedNotes:
                    logger.warning(f"Object `{wpr.obj.name}` with MIDI note `{noteNumber}` does not exist in the MIDI track provided (MIDI track `{self.midiTrack.name}`)!")

                if noteNumber in self.noteToWpr:
                    self.noteToWpr[noteNumber].append(wpr)
                else:
                    self.noteToWpr[noteNumber] = [wpr]
    
    def preAnimate(self):
        """deletes all old projectiles & their associated keyframes"""
        cleanCollection(self.projectileCollection, self.referenceProjectile)

    def animate(self):
        """generates projectiles with keyframes and adds them to the projectileCollection

        :raises ValueError: if the animation projectile object on the funnels do not have an animation curve (for the ball path)
        """
        # iterate over all notes
        for note in self.midiTrack.notes:
            # lookup blender object
            if note.noteNumber in self.noteToWpr:
                wprs = self.noteToWpr[note.noteNumber]
            else:
                continue
            
            # iterate over all "wrapped" Blender objects
            for wpr in wprs:
                obj = wpr.obj

                hit = obj.midi.hit_time
    
                frame = secToFrames(note.timeOn)
                
                # only needed because we are not using NoteOff at all
                # and the instrument validation checks both for NoteOn and NoteOff, and it only cares if at least 1 is present
                # this could be FIXME'd by creating more functionality for validateFCurves()
                # for now, this is okay
                if (wpr.startFrame, wpr.endFrame) == (None, None):
                    raise ValueError(f"Refrerence Projectile object `{obj.name}` has no Animation FCurves!")

                startFrame = wpr.startFrame - hit + frame
                endFrame = wpr.endFrame - hit + frame

                self._cache.addObject(FrameRange(startFrame, endFrame, wpr))
                
        
        # instance all projectiles and apply the keyframes by copying them and adding the frame range data
        cacheDict = self._cache.getCache()
        for cacheIndex in cacheDict:
            # instance the projectile
            projectileObj = self.referenceProjectile.copy()
            projectileObj.name = f"projectile_{hex(id(self.projectileCollection))}_{cacheIndex}"
            
            # hide them
            # 1 frame before so when they're turned on, it isn't overwritten
            showHideObj(obj=projectileObj, hide=True, frame=self._cache.getStartTime() - 1)
                
            self.projectileCollection.objects.link(projectileObj)
            
            # apply keyframes to projectiles now
            frameRanges = cacheDict[cacheIndex]
            for frameRange in frameRanges:
                # turn ball on
                showHideObj(obj=projectileObj, hide=False, frame=frameRange.startFrame)

                # insert keyframes associated with projectile
                for fCrv in frameRange.wpr.noteOnCurves:
                    for keyframe in fCrv.keyframe_points:
                        frame, value = keyframe.co

                        if fCrv.data_path == "location":
                            if self.collection.midi.use_initial_location:
                                value += frameRange.wpr.initalLoc[fCrv.array_index]

                            if self.collection.midi.use_circle_loc_map and fCrv.array_index == 0:
                                ## TODO this will need to be cleaned up & refactored, needs to be more dynamic & have less hardcoded values

                                index = list(self.noteToWpr.keys()).index(frameRange.wpr.noteNumbers[0])
                                funnelObject = frameRange.wpr.obj
                                angleObject = list(self.collection.midi.loc_collection.all_objects)[index]

                                # vibraphone divide value = /10.45
                                # marimba divide value = /3.7
                                # need to figure out what the divide issue
                                x, y = animateAlongTwoPoints(funnelObject.matrix_world.translation, angleObject.matrix_world.translation, keyframe.co[0]/10.45)
                                
                                # set the value to the new x value
                                value = x
                                
                                # set the y value & keyframe it (this is done because the y value is not animated on the reference object)
                                projectileObj.location.y = y
                                projectileObj.keyframe_insert(data_path="location", index=1, frame=frame + frameRange.startFrame)
                                setKeyframeInterpolation(projectileObj, "BEZIER", data_path=fCrv.data_path, array_index=1)
                                copyKeyframeProperties(obj=projectileObj, keyframeToCopy=keyframe, data_path=fCrv.data_path, array_index=1)

                        frame += frameRange.startFrame

                        exec(f"bpy.data.objects['{projectileObj.name}'].{fCrv.data_path}[{fCrv.array_index}] = {value}")
                        projectileObj.keyframe_insert(data_path=fCrv.data_path, index=fCrv.array_index, frame=frame)
                        
                        # make all keyframes bezier
                        setKeyframeInterpolation(projectileObj, "BEZIER", data_path=fCrv.data_path, array_index=fCrv.array_index)
                        copyKeyframeProperties(obj=projectileObj, keyframeToCopy=keyframe, data_path=fCrv.data_path, array_index=fCrv.array_index)

                # make the last keyframe constant
                setKeyframeInterpolation(projectileObj, "CONSTANT")

                # turn ball off
                showHideObj(obj=projectileObj, hide=True, frame=frameRange.endFrame)


class EvaluateInstrument(Instrument):
    noteToWpr: Dict[int, bpy.types.Object]

    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection):
        """Takes an object with FCurves and duplicates it across the timeline.

        :param MIDITrack midiTrack: the MIDITrack object to animate from
        :param bpy.types.Collection collection: the `bpy.types.Collection` of Blender objects to apply keyframes to
        """
        super().__init__(midiTrack=midiTrack, collection=collection)
        
        self.preAnimate()

        self.noteToWpr = dict()
        
        # ensure objects of keyframed type have either note on or note off FCurve objects
        for obj in self.collection.all_objects:
            if obj.midi.anim_type == "keyframed" and obj.midi.note_on_curve is None and obj.midi.note_off_curve is None:
                raise ValueError("Animation type `keyframed` must have either a Note On Curve or a Note Off Curve!")
        
        self.createNoteToObjectWpr()

    @staticmethod
    def drawInstrument(context: bpy.types.Context, col: bpy.types.UILayout, blCol: bpy.types.Collection,):
        """draws the UI for the instrument view"""
        pass
    
    @staticmethod
    def drawObject(context: bpy.types.Context, col: bpy.types.UILayout, blObj: bpy.types.Object):
        """draws the UI for the object view"""
        
        objMidi = blObj.midi

        col.separator()
        row0 = col.row()

        row0.prop(objMidi, "anim_type")

        if objMidi.anim_type == "damp_osc":
            col.prop(objMidi, "osc_period")
            col.prop(objMidi, "osc_amp")
            col.prop(objMidi, "osc_damp")

        elif objMidi.anim_type == "keyframed":
            row1 = col.row()
            row1.prop(objMidi, "note_on_curve", text="Note On")
            # this is temporary until I determine a solution for each FCurve
            row1.prop(objMidi, "note_on_anchor_pt", text="")

            row2 = col.row()
            row2.prop(objMidi, "note_off_curve", text="Note Off")
            row2.prop(objMidi, "note_off_anchor_pt", text="")


        elif objMidi.anim_type == "adsr":
            col.label(text="Coming soon")
        
        col.separator()

        col.prop(objMidi, "x_mapper")
        col.prop(objMidi, "y_mapper")

        col.prop(objMidi, "velocity_intensity", slider=True)

        col.prop(objMidi, "anim_overlap")

    @staticmethod
    def properties():
        """create properties for the instrument"""
        MIDIAnimatorObjectProperties.velocity_intensity = bpy.props.FloatProperty(
            name="Velocity Intensity", 
            description="The strength of the velocity affecting the keyframe values. 0 for no velocity influence.",
            default=0.0,
            soft_min=0,
            soft_max=2,
            options=set()
        )
        MIDIAnimatorObjectProperties.note_number_int = bpy.props.IntProperty(
            name="Note Number Integer", 
            description="The note number (integer) of an object",
            default=60
        )
        MIDIAnimatorObjectProperties.note_on_curve = bpy.props.PointerProperty(
            name="Note On Animation Curve", 
            description="The animation curve object with defined keyframes to be read in",
            type=bpy.types.Object,
            options=set()
        )
        MIDIAnimatorObjectProperties.note_on_anchor_pt = bpy.props.IntProperty(
            name="Note On Anchor Point",
            description="Where along should we start animating (in reference to the note on time). 0 to start animating right on note on times, - for eariler, + for later",
            default=0,
            options=set()
        )
        MIDIAnimatorObjectProperties.note_off_curve = bpy.props.PointerProperty(
            name="Note Off Animation Curve", 
            description="The animation curve object with defined keyframes to be read in."
                        "\n\nDisabled: will be added in a future release",
            type=bpy.types.Object,
            options=set()
        )
        MIDIAnimatorObjectProperties.note_off_anchor_pt = bpy.props.IntProperty(
            name="Note Off Anchor Point",
            description="Where along should we start animating (in reference to the note on time). 0 to start animating right on note off times, - for eariler, + for later."
                        "\n\nDisabled: will be added in a future release",
            default=0,
            options=set()
        )
        MIDIAnimatorObjectProperties.osc_period = bpy.props.FloatProperty(
            name="Period",
            description="Period of the oscillation",
            default=4,
            options=set()
        )
        MIDIAnimatorObjectProperties.osc_amp = bpy.props.FloatProperty(
            name="Amplitude",
            description="Amplitude of the oscillation",
            default=4,
            options=set()
        )
        MIDIAnimatorObjectProperties.osc_damp = bpy.props.FloatProperty(
            name="Damping",
            description="Damping of the oscillation",
            default=10,
            options=set()
        )
        MIDIAnimatorObjectProperties.anim_overlap = bpy.props.EnumProperty(
            items=[
                ("add", "Add", "Curves will add motion. More options will be added in the future")
            ],
            name="Animation Overlap",
            default="add",
            options=set()
        )
        MIDIAnimatorObjectProperties.anim_type = bpy.props.EnumProperty(
            items=[
                ("keyframed", "Keyframed", "Pre-defined FCurve objects to refernce the animation from"),
                ("damp_osc", "Oscillation", "Dampened oscillation. Planned for a future release"),
                ("adsr", "ADSR", "Attack, Decay, Sustain, Release. Planned for a future release")
            ],
            name="Animation Type",
            default="keyframed",
            options=set()
        )
        MIDIAnimatorObjectProperties.x_mapper = bpy.props.StringProperty(
            name="Time mapper",
            description="Time mapper (X axis) of the animation. Use 'x' for time, 'note' for note number, and 'vel' for velocity.",
            default="x",
            options=set()
        )
        MIDIAnimatorObjectProperties.y_mapper = bpy.props.StringProperty(
            name="Amplitude mapper",
            description="Amplitude mapper (Y axis) of the animation. Use 'y' for amplitude, 'note' for note number, and 'vel' for velocity.",
            default="y",
            options=set()
        )

    
    def createNoteToObjectWpr(self) -> None:
        """takes the MIDI file and builds a dictionary with the key being note number, and the value being a ObjectWrapper (which gets its FCurves)

        :raises ValueError: if there is no note number on the object
        """
        allUsedNotes = self.midiTrack.allUsedNotes()

        for obj in self.collection.all_objects:
            if obj.midi.note_number is None or not obj.midi.note_number: raise ValueError(f"Object '{obj.name}' has no note number!")

            # make sure objects are not in target collection
            assert not any(item in set((obj.midi.note_on_curve, obj.midi.note_off_curve)) for item in set(self.collection.all_objects)), "Animation reference objects are in the target animation collection! Please move them out of the collection."

            wpr = ObjectWrapper(
                obj=obj, 
                noteNumbers=convertNoteNumbers(obj.midi.note_number), 
                noteOnCurves=Instrument.getFCurves(animationObject=obj, referenceObject=obj.midi.note_on_curve), 
                noteOffCurves=Instrument.getFCurves(animationObject=obj, referenceObject=obj.midi.note_off_curve)
            )
            
            for noteNumber in wpr.noteNumbers:
                if noteNumber not in allUsedNotes:
                    logger.warning(f"Object `{wpr.obj.name}` with MIDI note `{noteNumber}` does not exist in the MIDI track provided (MIDI track `{self.midiTrack.name}`)!")

                if noteNumber in self.noteToWpr:
                    self.noteToWpr[noteNumber].append(wpr)
                else:
                    self.noteToWpr[noteNumber] = [wpr]

    def preAnimate(self):
        """cleans all keyframes before the animation"""
        bpy.context.scene.frame_set(-10000)
        for obj in self.collection.all_objects:
            cleanKeyframes(obj)
    
    def animate(self):
        """applys keyframe data to the objects from the MIDITrack"""
        def processNextKeys(curve, note, wpr, nextKeys):
            if isinstance(curve, bpy.types.FCurve):
                keyframes = curve.keyframe_points
            elif isinstance(curve, ObjectShapeKey):
                keyframes = curve.referenceCurve.keyframe_points
            else:
                # invaid curve type
                return

            for i, keyframe in enumerate(keyframes):
                # get offsets for this note
                if curve == noteOnCurve:
                    offset = secToFrames(note.timeOn) + wpr.obj.midi.note_on_anchor_pt
                elif curve == noteOffCurve:
                    offset = secToFrames(note.timeOff) + wpr.obj.midi.note_off_anchor_pt
    
                frame = keyframe.co[0]
                value = keyframe.co[1]

                # X and Y mappers
                frame = eval(str(wpr.obj.midi.x_mapper).replace("x", f"{frame}").replace("note", f"{note.noteNumber}").replace("vel", f"{note.velocity}") if wpr.obj.midi.x_mapper else f"{frame}")
                value = eval(str(wpr.obj.midi.y_mapper).replace("y", f"{value}").replace("note", f"{note.noteNumber}").replace("vel", f"{note.velocity}") if wpr.obj.midi.y_mapper else f"{value}")
                
                frame += offset

                if wpr.obj.midi.velocity_intensity != 0:
                    value *= note.velocity / 127 * wpr.obj.midi.velocity_intensity
                

                # this is a way to convert Blender's Elastic interpolation to straight keyframes
                # currently this is limited to only 1 elastic animation (must be the first one), and it must be a ease out animation
                # eventually i will support more but this is okay for now, will be replaced with a better system in the future
                if i == 0 and keyframe.interpolation == "ELASTIC":
                    lengthToNextKey = curve.keyframe_points[i+1].co[0] - keyframe.co[0]
                    
                    period = 6.5 / keyframe.period
                    amplitude = keyframe.amplitude * 1.2
                    damp = 8 / lengthToNextKey
                    
                    period = eval(str(wpr.obj.midi.x_mapper).replace("x", f"{period}").replace("note", f"{note.noteNumber}").replace("vel", f"{note.velocity}") if wpr.obj.midi.x_mapper else f"{period}")
                    amplitude = eval(str(wpr.obj.midi.y_mapper).replace("y", f"{amplitude}").replace("note", f"{note.noteNumber}").replace("vel", f"{note.velocity}") if wpr.obj.midi.y_mapper else f"{amplitude}")

                    generatedOscKeys = genDampedOscKeyframes(period, amplitude, damp, keyframe.co[0] + offset)
                    nextKeys.extend(generatedOscKeys)
                else:
                    nextKeys.append(Keyframe(frame, value))


        # create wprToKeyframe
        wprToKeyframe  = {}
        for noteNumber in self.noteToWpr:
            wprs = self.noteToWpr[noteNumber]
            
            for wpr in wprs:
                wprToKeyframe[wpr] = {}
        

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
                    for (noteOnCurve, noteOffCurve) in zip_longest(wpr.noteOnCurves, wpr.noteOffCurves):
                        nextKeys = []

                        # there will never be a time where both noteOnCurve and noteOffCurve will be None
                        # this precondition is checked when creating the wrapper Blender objects
                        if noteOnCurve:
                            # find the keyframe lists for this particular FCurve
                            key = (noteOnCurve.data_path, noteOnCurve.array_index)
                        elif noteOffCurve:
                            # try using noteOffCurve if there is no noteOnCurve
                            key = (noteOffCurve.data_path, noteOffCurve.array_index)

                        if key not in wprToKeyframe[wpr]:
                            wprToKeyframe[wpr][key] = []
                        
                        keyframes = wprToKeyframe[wpr][key]

                        # process next keys
                        if wpr.noteOnCurves:
                            processNextKeys(noteOnCurve, note, wpr, nextKeys)

                        if wpr.noteOffCurves:
                            processNextKeys(noteOffCurve, note, wpr, nextKeys)

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
                        fCrv = noteOnCurve if noteOnCurve is not None else noteOffCurve
                        
                        # if the object does not play anything (no MIDI notes read == no keyframes to write)
                        if (fCrv.data_path, fCrv.array_index) not in wprToKeyframe[wpr]: continue
                        keyframes = wprToKeyframe[wpr][(fCrv.data_path, fCrv.array_index)]
                        # keyframes = wpr.keyframes.listOfKeys[(fCrv.data_path, fCrv.array_index)]
                        
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

# ------------------------------------------------------------------

@dataclass
class InstrumentItem:
    identifier: str
    name: str
    description: str
    cls: Type[Instrument]


# Enum of instruments. Add new instruments here and they will be automatically added to the UI
class Instruments(Enum):
    projectile = InstrumentItem(identifier="projectile", name="Projectile", description="A projectile that is fired from the instrument", cls=ProjectileInstrument)
    evaluate = InstrumentItem(identifier="evaluate", name="Evaluate", description="Evaluate thing", cls=EvaluateInstrument)
    custom = InstrumentItem(identifier="custom", name="Custom", description="Custom Instrument. Must pass the class via `MIDIAnimatorNode.addInstrument()`. See the docs for help.", cls=Instrument)


# Shared properties throughout all objects, instruments(collections) and scences. 
# For example, a note number is a property likely used by all instruments, so it is defined here
class MIDIAnimatorObjectProperties(bpy.types.PropertyGroup):
    note_number:  bpy.props.StringProperty(
        name="Note Number", 
        description="The note number of an object. Can be entered as a integer (MIDI Note Number, e.g. 60) or as a "
                    "readable note (C3)",
        default="C3"
    )

class MIDIAnimatorCollectionProperties(bpy.types.PropertyGroup):
    instrument_type: bpy.props.EnumProperty(
        items=[(item.value.identifier, item.value.name, item.value.description) for item in Instruments],
        name="Instrument Type",
        default="evaluate",
        options=set()
    )

class MIDIAnimatorSceneProperties(bpy.types.PropertyGroup):
    # Edit Notes (Quick)
    quick_note_number_list: bpy.props.StringProperty(
        name="Note Number List",
        description="A list of note numbers. These will correspond to the objects in the selected collection",
        default="[]",
        options=set()
    )
    quick_obj_col: bpy.props.PointerProperty(
        type=bpy.types.Collection, 
        name="Collection",
    )
    quick_sort_by_name: bpy.props.BoolProperty(
        name="Sort by Name",
        description="This will use a sorted list of objects by name instead of using `name_noteNumber`",
        default=False,
        options=set()
    )