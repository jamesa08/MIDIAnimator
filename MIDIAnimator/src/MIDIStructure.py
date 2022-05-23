from .. utils.functions import noteToName, nameToNote, removeDuplicates
from .. utils.animation import *
from typing import Tuple, Dict, List
import bpy

class MIDINote:
    # number
    # structure (channel, note, velocity, time)

    number: int
    onTime: float
    offTime: float  # ignore to start
    velocity: int
    channel: int

    def __init__(self):
        pass

class MIDITrack:
    # make MIDINote object?
    name: str

    # structure (channel, note, velocity, time)

    _notesToPlay: List[MIDINote]

    noteOn: List[Tuple[int, int, int, float]]
    noteOff: List[Tuple[int, int, int, float]]
    
    # maps control number to control change events
    # {control_change_number: [(channel, value, time), ...]}
    # class?
    controlChange: Dict[int, List[Tuple[int, float, float]]]
    
    # (channel, value, time)
    pitchwheel: List[Tuple[int, float, float]]
    
    # (channel, value, time)
    aftertouch: List[Tuple[int, float, float]]

    def __init__(self, name: str):
        """initialize a MIDITrack

        :param str name: name of track
        """
        # metadata
        self.name = name
        
        # note on event
        # structure (channel, note, velocity, time)
        self.noteOn = []

        # note off event
        # structure (channel, note, velocity, time)
        self.noteOff = []

        # control changes
        # structure: dict(control_change_number: [(channel, value, time), ...])
        self.controlChange = {}
        
        # pitch wheel
        # structure: (channel, value, time)
        self.pitchwheel = []

        # aftertouch
        # structure: (channel, value, time)
        self.aftertouch = []

    # def combineNoteOnOffLists(self):
    #     activeNotes = []
    #     noteOnIndex = 0
    #     noteOffIndex = 0
    #     while noteOnIndex < len(self.noteOn):

    def addNoteOn(self, channel: int, noteNumber: int, velocity: int, timeOn: float) -> None:
        """adds a Note Event

        :param int channel: the MIDI Channel the note is on
        :param int noteNumber: the note number, range from 0-127
        :param int velocity: the note velocity, range 0-127
        :param float timeOn: the note time on, in seconds
        """
        self.noteOn.append((channel, noteNumber, velocity, timeOn))

    def addNoteOff(self, channel: int, noteNumber: int, velocity: int, timeOff: float) -> None:
        """adds a Note Off event

        :param int channel: MIDI channel
        :param int noteNumber: the note number, TODO range
        :param int velocity: the note velocity, TODO range
        :param float timeOff: the note time off, in seconds
        """
        self.noteOff.append((channel, noteNumber, velocity, timeOff))

    def addControlChange(self, control_number: int, channel: int, value: int, time: float):
        """add a control change value
        automatically checks if number has been added

        :param int control_number: the control change number
        :param int channel: MIDI channel number
        :param int value: value of the control change
        :param float time: time value (in seconds)
        """
        if control_number in self.controlChange:
            # in dict
            self.controlChange[control_number].append((channel, value, time))
        else:
            # not in dict
            self.controlChange[control_number] = [(channel, value, time)]

    def addPitchwheel(self, channel: int, value: float, time: float) -> None:
        """add a pitchwheel event

        :param int channel: the MIDI channel number
        :param float value: value of the pitch wheel TODO range
        :param float time: time value (in seconds)
        """
        self.pitchwheel.append((channel, value, time))

    def addAftertouch(self, channel: int, value: float, time: float) -> None:
        """add a aftertouch event

        :param int channel: the MIDI channel number
        :param float value: value of the aftertouch, TODO range
        :param float time: time value (in seconds)
        """
        self.aftertouch.append((channel, value, time))

    def _checkIfEqual(self) -> bool:
        """checks if timesOn, timesOff and noteNumbers are of equal length

        :return bool: returns True if list are of equal length, False otherwise
        """
        return len(self.noteOn) == len(self.noteOff)

    def _isEmpty(self) -> bool:
        """checks if MIDITrack is empty

        :return bool: returns True if MIDITrack is empty, False otherwise
        """

        variables = [self.noteOn, self.noteOff, self.controlChange, self.pitchwheel, self.aftertouch]

        return len(variables) == sum([len(v) == 0 for v in variables])

    def allUsedNotes(self) -> list:
        """Returns a list of all used notes in a MIDI Track.

        :return list: a list of all used notes
        """

        return removeDuplicates([num[1] for num in self.noteOn])

    def postProcess(self):
        pass
        # build up _notesToPlay list of MIDINote() objects and store in an instance var
        # sorted by timeOn
        # clear out old lists using .clear()

    def __str__(self) -> str:
        # TODO: Refactor & optimize
        # Not Pythonic!

        out = [f"MIDITrack(name'{self.name}', \nnoteOn=[\n\t"]

        for i, (channel, note, velocity, time) in enumerate(self.noteOn):
            out.append(f"NoteOn(channel={channel}, note={note}, velocity={velocity}, time={time})")
            if i != len(self.noteOn) - 1:
                out.append(",\n\t")

        out.append("],\nnoteOffs=[\n\t")

        for i, (channel, note, velocity, time) in enumerate(self.noteOff):
            out.append(f"NoteOff(channel={channel}, note={note}, velocity={velocity}, time={time})")
            if i != len(self.noteOff) - 1:
                out.append(",\n\t")
        
        out.append("],\ncontrolChanges={\n\t")

        for i, key in enumerate(self.controlChange):
            out.append(f"control_number={key}: ")
            for j, (channel, value, time) in enumerate(self.controlChange[key]):
                out.append(f"(channel={channel}, value={value}, time={time})")
                
                if j != len(self.controlChange[key]) - 1:
                    out.append(",\n\t\t")
        
            if i != len(self.controlChange) - 1:
                out.append(",\n\t")

        out.append("},\nafterTouch=[\n\t")

        for i, (channel, value, time) in enumerate(self.aftertouch):
            out.append(f"Aftertouch(channel={channel}, value={value}, time={time})")
            if i != len(self.aftertouch) - 1:
                out.append(",\n\t")

        out.append("],\npitchwheel=[\n\t")

        for i, (channel, value, time) in enumerate(self.pitchwheel):
            out.append(f"Pitchwheel(channel={channel}, value={value}, time={time})")
            if i != len(self.pitchwheel) - 1:
                out.append(",\n\t")

        out.append("]\n)")
        
        return "".join(out)

    def __repr__(self) -> str:
        type_ = type(self)
        module = type_.__module__
        qualname = type_.__qualname__

        return f"<{module}.{qualname} object \"{self.name}\", at {hex(id(self))}>"


class BlenderTrack:
    _midiTrack: MIDITrack
    _noteToBlender: Dict[int, AnimatableBlenderObject] # key is the note's number

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