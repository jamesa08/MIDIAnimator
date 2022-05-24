from . MIDIStructure import MIDIFile, MIDITrack, MIDINote
from typing import List, Tuple, Dict
import bpy


class ProjectileCache:
    pass

class AnimatableBlenderObject:
    _blenderObject: bpy.types.Object
    _note: MIDINote
    _projectile: bpy.types.Object
    _startFrame: int
    _endFrame: int
    _controlPoints: Tuple[float, float, float]

    def __init__(self, note: MIDINote, blenderCollection: bpy.types.Collection):
        self._note = note
        # determine _blenderObject using bpy and the note

    def calculateDataForNoteHitTime(self, hitTime: float):
        pass

    def startFrame(self) -> int:
        pass

    def endFrame(self) -> int:
        return self._endFrame

    def _controlPoints(self):
        pass

    def positionForFrame(self, frameNumber: int) -> Tuple[float, float, float]:
        pass

class BlenderTrack:
    _midiTrack: MIDITrack
    _noteToBlender: Dict[int, AnimatableBlenderObject]  # key is the note's number

    def __init__(self, midiTrack: MIDITrack):
        pass

    def setInstrument(self, instrumentType: str, objectCollection: bpy.types.Collection):
        pass
        # iterate over objectCollection and get the note_numbers and FCurve data
        # build up a dictionary with the note as the key and the value will be a new AnimatableBlenderObject() or a sub-
        # class depending on the instrumentType parameter

    def computeStartEndFramesForObjects(self) -> List[Tuple[int, int, AnimatableBlenderObject]]:
        """
        returns: list of tuples (startFrame, endFrame, AnimatableBlenderObject)
        """
        pass

        #    make empty list
        #    for each note in sorted order (track._notesToPlay)
        #        animatableObject = use track dictionary that gets it for the note (using the note number as the key)
        #        animatableObject.calculateDataForNoteHitTime(note.onTime)
        #        add tuple (animatableObject.startFrame(), animatableObject.endFrame, animatableObject) to list
        #     return list


class BlenderObjectProjectile(AnimatableBlenderObject):

    def calculateDataForNoteHitTime(self, hitTime: float):
        pass
        # calculate other instance variables - startFrame, endFrame based on FCurve
        # for example if the note is to be played at frame 100
        # the FCurve stores offset at which the note is hit (metadata)
        # so the startFrame is note.onTime minus that offset
        # calculate the endFrame based on the last controlPoint and the startFrame

    def _getReusableProjectile(self, cache: ProjectileCache):
        pass

class BlenderObjectString(AnimatableBlenderObject):
    pass

class BlenderAnimation:
    _blenderTracks: List[BlenderTrack]

    def __init__(self, midiFile: MIDIFile):
        # call MIDIFile and read in
        self._blenderTracks = []

    def setInstrumentForTrack(self, instrumentType: str, track: MIDITrack, objectCollection: bpy.types.Collection):
        """add an instrument definition
        :param str instrumentType: midiType of animation. See docs
        Custom allows you to write your own animation driver code. See the docs for more info
        :param MIDITrack track: the MIDITrack object. see docs
        :param bpy.data.collection objectCollection: the object collection to be animated. Must be a `bpy collection`.
        """

        # make a new BlenderTrack and add to _blenderTracks
        blenderTrack = BlenderTrack(track)
        blenderTrack.setInstrument(instrumentType, objectCollection)
        self._blenderTracks.append(blenderTrack)

        if instrumentType == "projectile":
            pass
            # create BlenderObjectProjectile()
            # get all notes for track
            # map each note to object and make AnimatableBlenderObject for each note (dict?)
            # calculate number of needed projectiles & instance the blender objects using bpy

    def animate(self) -> None:
        pass

        # create an empty list (combined list)
        # for each blender track
        #    data = track.computeStartEndFramesForObjects
        #    data is a list [(startFrame, endFrame, AnimatableBlenderObject), ...]
        #
        #     merge this list into combined list
        #  [2, 6, 8, 9] [1, 3, 5, 10, 11]

        # initialize empty list of activeObjectList AnimatableBlenderObject (objects that move)
        # for each frame
        #     remove from list of active objects and objects whose end frame is before this frame
        #     update the list of active objects (add any new objects from combined list whose startFrame is this frame)
        #
        #     make an empty list for newlyInsertedItems
        #     for item in activeObjectList
        #         if item causes new items to move at its hit time, insert the other item into newlyInsertedItems
        #         call AnimatableBlenderObject positionForFrame method
        #     for each newlyInsertItems
        #         call AnimatableBlenderObject positionForFrame method
        #     activeObjectList.extend(newlyInsertedItems)
        #     tell Blender to render the frame

