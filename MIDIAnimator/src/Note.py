from __future__ import annotations
from typing import List
import bpy

class Note:
    _noteNumber: int

    def __init__(self, noteNumber: int):
        self._noteNumber = noteNumber
        # dictionary of each note with its time on, time off, etc.

    def __str__(self) -> str:
        pass
        # note number (evantually add instrument info) or use the util function to get note name

class PlayedNote:
    _note: Note
    _startTime: float
    _endTime: float
    _velocity: float

    def __str__(self, note: Note, startTime: float, endTime: float, velocity: float) -> str:
        pass
        # use str(self._note) and also include start, end, velocity

class BlenderObject:
    # can we ask the Blender API for a list of all the objects (in some collection)
    _blenderID: List[str, int]

class BlenderNoteObject(BlenderObject):
    _noteNumber: int

    @staticmethod
    def getNoteObjects(parameterIfNecessaryToIdentifyBlenderFile) -> List[BlenderNoteObject]:
        pass

    # @staticmethod
    # def createProjectiles(numProjectils: int) -> List[BlenderObjectID]
    #     pass
        