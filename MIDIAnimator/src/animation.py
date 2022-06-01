from math import comb
from . MIDIStructure import MIDIFile, MIDITrack, MIDINote
from .. utils.functions import noteToName
from .. utils.animation import FCurveLength, FCurvesFromObject, secToFrames
from typing import List, Tuple, Dict
import bpy


class ProjectileCache:
    pass

class AnimatableBlenderObject:
    _blenderObject: bpy.types.Object
    _note: int
    _projectile: bpy.types.Object
    _startFrame: int
    _endFrame: int
    _controlPoints: Tuple[float, float, float]

    def __init__(self, obj: bpy.types.Object):
        if obj.note_number is None: raise RuntimeError(f"Object {obj} has no note number!")
        if obj.animation_curve is None: raise RuntimeError(f"Object {obj} has no animation curve!")
        
        # determine _blenderObject using bpy and the note
        self._blenderObject = obj
        self._note = int(obj.note_number)
        self._FCurve = FCurvesFromObject(obj.animation_curve)[obj.animation_curve_index]

        self._startFrame = None
        self._endFrame = None

    def calculateDataForNoteHitTime(self, timeOn: float):
        pass

    def startFrame(self) -> int:
        return self._startFrame

    def endFrame(self) -> int:
        return self._endFrame

    def _controlPoints(self):
        pass

    def positionForFrame(self, frameNumber: int) -> Tuple[float, float, float]:
        # eval FCurve
        # TODO: VERY TEMP
        return FCurvesFromObject(self._blenderObject.animation_curve)[0].evaluate(frameNumber), FCurvesFromObject(self._blenderObject.animation_curve)[1].evaluate(frameNumber)

class BlenderObjectProjectile(AnimatableBlenderObject):

    def calculateDataForNoteHitTime(self, timeOn: float):
        # calculate other instance variables - startFrame, endFrame based on FCurve
        # for example if the note is to be played at frame 100

        # the FCurve stores offset at which the note is hit (metadata)
        # so the startFrame is note.timeOn minus that offset
        # calculate the endFrame based on the last controlPoint and the startFrame

        hit = self._blenderObject.note_hit_time
        rangeVector = self._FCurve.range()
        startFCurve, endFCurve = rangeVector[0], rangeVector[1]
        
        frame = secToFrames(timeOn)
        self._startFrame = (startFCurve - hit) + frame
        self._endFrame = (endFCurve - hit) + frame


    def _getReusableProjectile(self, cache: ProjectileCache):
        pass



class BlenderObjectString(AnimatableBlenderObject):
    
    def calculateDataForNoteHitTime(self, timeOn: float):
        # calculate other instance variables - startFrame, endFrame based on FCurve
        # for example if the note is to be played at frame 100

        # the FCurve stores offset at which the note is hit (metadata)
        # so the startFrame is note.timeOn minus that offset
        # calculate the endFrame based on the last controlPoint and the startFrame

        hit = self._blenderObject.note_hit_time

        rangeVector = self._FCurve.range()
        startFCurve, endFCurve = rangeVector[0], rangeVector[1]
        
        frame = secToFrames(timeOn)
        self._startFrame = (startFCurve - hit) + frame
        self._endFrame = (endFCurve - hit) + frame



class BlenderTrack:
    
    _midiTrack: MIDITrack
    _noteToBlender: Dict[int, Tuple[bpy.types.Object, AnimatableBlenderObject]]  # key is the note's number

    def __init__(self, midiTrack: MIDITrack):
        self._midiTrack = midiTrack
        self._noteToBlender = dict()

    def setInstrument(self, col: bpy.types.Collection, projectileCollection: bpy.types.Collection):
        # iterate over objectCollection and get the note_numbers and FCurve data
        # build up a dictionary with the note as the key and the value will be a new AnimatableBlenderObject() or a sub-
        # class depending on the instrumentType parameter

        for obj in col.all_objects:
            if obj.note_number is None: raise RuntimeError(f"Object {obj} has no note number!")

            # vals = (obj, obj.note_number, obj.animation_curve[obj.animation_curve_index])
            if col.instrument_type == "string":
                cls = BlenderObjectString(obj)
            elif col.instrument_type == "projectile":
                if projectileCollection is None: raise ValueError("Projectile Collection for instrument type Projectile is not passed!")
                cls = BlenderObjectProjectile(obj)
            
            vals = (obj, cls)
            
            if obj.note_number in self._noteToBlender:
                # key in dict
                self._noteToBlender[obj.note_number].append(vals)
            else:
                # key not in dict
                self._noteToBlender[obj.note_number] = [vals]

    def computeStartEndFramesForObjects(self) -> List[Tuple[int, int, AnimatableBlenderObject]]:
        """
        returns: list of tuples (startFrame, endFrame, AnimatableBlenderObject)
        """
        #    make empty list
        #    for each note in sorted order (track._notesToPlay)  # already sorted
        #        animatableObject = use track dictionary that gets it for the note (using the note number as the key)
        #        animatableObject.calculateDataForNoteHitTime(note.timeOn)
        #        add tuple (animatableObject.startFrame(), .endFrame, animatableObject) to list
        #     return list
        
        out = []
        
        assert self._noteToBlender is not None

        for note in self._midiTrack.notes:
            assert self._noteToBlender[str(note.noteNumber)] is not None, f"Note {note.noteNumber}/{noteToName(note.noteNumber)} in MIDITrack has no Blender object to map to!"

            for animatableObject in self._noteToBlender[str(note.noteNumber)]:
                obj, cls = animatableObject
                cls.calculateDataForNoteHitTime(note.timeOn)
                out.append((cls.startFrame(), cls.endFrame(), cls))

        return out


# these are the objects you would instance in
class BlenderAnimation:
    _blenderTracks: List[BlenderTrack]

    def __init__(self):
        # call MIDIFile and read in  # if we want the user to be able to access the tracks & pass them in, then they should instance MIDIFile themselves
        self._blenderTracks = []

    def setInstrumentForTrack(self, track: MIDITrack, objectCollection: bpy.types.Collection, projectileCollection: bpy.types.Collection=None, attribute: str=None):
        # make a new BlenderTrack and add to _blenderTracks

        # get all notes for track   # .setInstrument() does that
        # create BlenderObjectProjectile()  # .setInstrument() does that
        # map each note to object and make AnimatableBlenderObject for each note (dict?)  # .setInstrument() does that
        
        blenderTrack = BlenderTrack(track)
        blenderTrack.setInstrument(objectCollection, projectileCollection)
        self._blenderTracks.append(blenderTrack)

        insType = objectCollection.instrument_type

        # clean collection
        if insType == "projectile":
            self._cleanCollection(projectileCollection, "MAIN")  # TODO: remove hardcode "MAIN", let user define object to clone

        # calculate number of needed projectiles & instance the blender objects using bpy

        # TODO: figure out how many balls we need
        
        # TEMPORAY CODE 
        # need to figure out the max amount of balls visible in any frame
        # to make some balls for testing (make 1 ball per note)

        for key in blenderTrack._noteToBlender:  # key == note number
            listOfNotes = blenderTrack._noteToBlender[key]
            for obj, cls in listOfNotes:
                if insType == "projectile":
                    dup = bpy.data.objects["MAIN"].copy()
                    dup.name = f"{hex(id(cls))}_{key}"
                    projectileCollection.objects.link(dup)
                    
                
                
    def _cleanCollection(self, col: bpy.types.Collection, name: str) -> None:
        """
        Cleans a collection of old objects (to be reanimated)
        """
        assert len(col.all_objects) != 0, "Please have a projectile in the collection to clone."

        objsToRemove = []
        found = False

        for obj in col.all_objects:
            if obj.name == name: 
                # keep the object
                found = True  
            else:
                # remove
                objsToRemove.append(obj) 
        
        for obj in objsToRemove:
            bpy.data.objects.remove(obj, do_unlink=True)

        assert found is True, "Please provide a projectile in the collection to have cloned"
        assert len(col.all_objects) == 1


    def animate(self) -> None:

        # create an empty list (combined list)
        # for each blender track
        #    data = track.computeStartEndFramesForObjects
        #    data is a list [(startFrame, endFrame, AnimatableBlenderObject), ...]
        #
        #  [2, 6, 8, 9] [1, 3, 5, 10, 11]

        # merge this list into combined list
        combined = [track.computeStartEndFramesForObjects() for track in self._blenderTracks]
        
        # TODO: remove later when we merge multiple tracks
        combined = combined[0]

        print(combined)
        # sort by startFrame
        
        combined.sort(key=lambda data: data[0])
        combined.reverse()

        activeObjectList = []

        frameStart, frameEnd = bpy.context.scene.frame_start, bpy.context.scene.frame_end

        for frame in range(frameStart, frameEnd):
            # del objects not used only if list is not empty (otherwise will fail)
            if len(activeObjectList) == 0:
                # remove from list of active objects and objects whose end frame is before this frame
                activeObjectList = list(filter(lambda data: round(data[1]) >= frame, activeObjectList))
            
            # update the list of active objects (add any new objects from combined list whose startFrame is this frame)
            
            # for efficiency reasons, the list is reversed, so we start at the end
            i = len(combined) - 1

            # while we still have notes to play and the start time for this note is on or before this frame, then add it to activeObjectList
            while i >= 0 and combined[i][0] <= frame:
                activeObjectList.append(combined[i])
                
                # delete this note b/c we added it to active object list & move to next note 
                combined.pop()
                i -= 1
            

            # add keyframe here
            for frameOn, frameOff, cls in activeObjectList:
                # fill in x from object
                hitTime = cls._blenderObject.note_hit_time
                delta = frame - frameOn
                x = cls._blenderObject.location[0]
                y, z = cls.positionForFrame(delta)
                bpy.data.objects[f"{hex(id(cls))}_{cls._note}"].location = (x, y, z)
                bpy.data.objects[f"{hex(id(cls))}_{cls._note}"].keyframe_insert(data_path="location", frame=frame)
