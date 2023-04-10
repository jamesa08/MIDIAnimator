from __future__ import annotations

from typing import Dict, List, Union
from itertools import zip_longest
import bpy

from ..data_structures.midi import MIDITrack
from .. utils import convertNoteNumbers
from .. utils.loggerSetup import *
from .. data_structures import *
from .. utils.blender import *
from . algorithms import *

class Instrument:
    """base class for MIDI instruments. These will handle all pre-animation and animation techniques."""
    collection: bpy.types.Collection
    midiTrack: MIDITrack
    noteToWpr: Dict[int, bpy.types.Object]

    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, override=False):
        """Base class for MIDI instruments. These will handle all pre-animation and animation techniques.
        You should not instance this class by itself. This class should be inherited.

        :param MIDITrack midiTrack: the MIDITrack object to animate from
        :param bpy.types.Collection collection: the `bpy.types.Collection` of Blender objects to apply keyframes to
        :param bool override: override the __post_init__ method, defaults to False
        """
        self.collection = collection
        self.midiTrack = midiTrack
        self.noteToWpr = dict()
        self.override = override
        self.preAnimate()

        if not self.override:
            self.__post_init__()
    
    def __post_init__(self):
        """checks objects to see if they're valid
        creates note to ObjectWrapper dictionary

        :raises ValueError: _description_
        """
        # ensure objects of keyframed type have either note on or note off FCurve objects
        for obj in self.collection.all_objects:
            if obj.midi.anim_type == "keyframed" and obj.midi.note_on_curve is None and obj.midi.note_off_curve is None:
                raise ValueError("Animation type `keyframed` must have either a Note On Curve or a Note Off Curve!")
        
        self.createNoteToObjectWpr()

    @staticmethod
    def getFCurves(obj: bpy.types.Object, noteType: str="note_on") -> List[Union[bpy.types.FCurve, ObjectShapeKey]]:
        """gets the FCurves on a Blender Object.

        :param bpy.types.Object obj: the object with either a note_on_curve or a note_off_curve to get the FCurves from
        :param str noteType: which note type to get, can be `"note_on"` or `"note_off"`, defaults to `"note_on"`
        :raises ValueError: if `noteType` is invalid
        :return List[Union[bpy.types.FCurve, ObjectShapeKey]]: returns a list of either `bpy.types.FCurve`s or an `ObjectShapeKey`.
        """
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
                noteOnCurves=Instrument.getFCurves(obj=obj, noteType="note_on"), 
                noteOffCurves=Instrument.getFCurves(obj=obj, noteType="note_off")
            )
            
            for noteNumber in wpr.noteNumbers:
                if noteNumber not in allUsedNotes:
                    print(f"WARNING: Object `{wpr.obj.name}` with MIDI note `{noteNumber}` does not exist in the MIDI track provided (MIDI track `{self.midiTrack.name}`)!")

                if noteNumber in self.noteToWpr:
                    self.noteToWpr[noteNumber].append(wpr)
                else:
                    self.noteToWpr[noteNumber] = [wpr]

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
    _cache: CacheInstance

    def __init__(self, midiTrack: MIDITrack, objectCollection: bpy.types.Collection, projectileCollection: bpy.types.Collection, referenceProjectile: bpy.types.Object):
        """Pre-defined animation code that animates a projectile launching from a funnel.

        :param MIDITrack midiTrack: the MIDITrack object to animate from
        :param bpy.types.Collection objectCollection: the `bpy.types.Collection` of Blender objects (funnels) to apply keyframes to. These are the funnel objects (starting position for the projectiles).
        :param bpy.types.Collection projectileCollection: the `bpy.types.Collection` to store the projectiles
        :param bpy.types.Object referenceProjectile: the `bpy.types.Object` to clone the projectile to. This will not be animated, the mesh will be copied.
        """
        self._cache = CacheInstance()
        self.projectileCollection = projectileCollection
        self.referenceProjectile = referenceProjectile

        super().__init__(midiTrack, objectCollection)

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

                if obj.midi.anim_type == "keyframed":
                    hit = obj.midi.note_on_anchor_pt
        
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
                    
                elif obj.midi.anim_type == "osc":
                    pass
                elif obj.midi.anim_type == "adsr":
                    pass
        
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
                        frame += frameRange.startFrame

                        # if fCrv.data_path == "location":
                        #     value = wpr.initalLoc[fCrv.array_index]

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
    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection):
        """Takes an object with FCurves and duplicates it across the timeline.

        :param MIDITrack midiTrack: the MIDITrack object to animate from
        :param bpy.types.Collection collection: the `bpy.types.Collection` of Blender objects to apply keyframes to
        """
        super().__init__(midiTrack=midiTrack, collection=collection)

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
    
                frame = keyframe.co[0] + offset
                value = keyframe.co[1]

                if wpr.obj.midi.velocity_intensity != 0:
                    value *= note.velocity / 127 * wpr.obj.midi.velocity_intensity
                

                # this is a way to convert Blender's Elastic interpolation to straight keyframes
                # currently this is limited to only 1 elastic animation (must be the first one), and it must be a ease out animation
                # eventually i will support more but this is good enough for now
                if i == 0 and keyframe.interpolation == "ELASTIC":
                    lengthToNextKey = curve.keyframe_points[i+1].co[0] - keyframe.co[0]
                    
                    period = 6.5 / keyframe.period
                    amplitude = keyframe.amplitude * 1.2
                    damp = 8 / lengthToNextKey

                    generatedOscKeys = genDampedOscKeyframes(period, amplitude, damp, frame)
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
