from numpy import var
from .. utils.functions import noteToName, nameToNote, removeDuplicates
from typing import Tuple, Dict, List

class MIDITrack:
    name: str
    noteOn: Tuple[int, int, int, float]
    noteOff: Tuple[int, int, int, float]
    controlChange: Dict[int, List[Tuple[int, int, float]]]
    pitchwheel: Tuple[int, int, float]
    aftertouch: Tuple[int, int, float]

    
    def __init__(self, name: str):
        """intialize a MIDITrack

        :param str name: name of track
        :param int index: position of track (not used, remove?)
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

        for i, (channel, value, velocity) in enumerate(self.aftertouch):
            out.append(f"Aftertouch(channel={channel}, value={value}, time={time})")
            if i != len(self.aftertouch) - 1:
                    out.append(",\n\t")

        out.append("],\npitchwheel=[\n\t")

        for i, (channel, value, velocity,) in enumerate(self.pitchwheel):
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

# TODO: MIDIInstrument not needed?
# class MIDIInstrument:
#     def __init__(self, name: str):
#         # metadata
#         self.name = name
#         self.tracks = []
    

#     def addTrack(self, track: MIDITrack) -> MIDITrack:
#         """adds a track to MIDIInsturment

#         :param MIDITrack track: the MIDITrack to add
#         """
#         self.tracks.append(track)
#         return self.tracks[-1]


#     def addTracks(self, tracks: list, overwrite=False) -> list:
#         """add tracks to MIDIInstrument.

#         :param list tracks: list of MIDITracks
#         :param bool overwrite: overwrite existing tracks, defaults to False
#         """
#         if overwrite:
#             self.tracks = tracks
#         else:
#             for track in tracks:
#                 self.tracks.append(track)

#         return self.tracks
    
#     def remove(self, element: MIDITrack):
#         """deletes a MIDITrack from a MIDIInstrument

#         if a MIDITrack is not in the MIDIInstrument, a ValueError is thrown
#         :param MIDITrack element: the MIDITrack to delete
#         """
#         try:
#             self.tracks.remove(element)
#         except ValueError:
#             raise ValueError(f"MIDITrack {repr(element)} was not found in MIDIInstrument {repr(self)}.")

#     def _isEmpty(self) -> bool:
#         """checks if the MIDIInstrument is empty.

#         :return bool: returns True if empty, False otheriwse
#         """

#         for track in self.tracks:
#             if not track._isEmpty():
#                 return False
        
#         return True

#     def _cleanInstruments(self):
#         """removes empty tracks in a MIDIInstrument"""

#         for track in self.tracks:
#             if track._isEmpty():
#                 # delete the empty track
#                 self.remove(track)


#     def __str__(self) -> str:
#         out = [f"MIDIInstrument(name='{self.name}', index={self.index}, tracks=["]
        
#         for i, track in enumerate(self.tracks):
#             out.append(str(track))

#             if i != len(self.tracks) - 1:
#                 out.append(", ")

#         out.append("])")

#         return "".join(out)
    

#     def __repr__(self) -> str:
#         type_ = type(self)
#         module = type_.__module__
#         qualname = type_.__qualname__

#         return f"<{module}.{qualname} object \"{self.name}\", at {hex(id(self))}>"


#     def __iter__(self):
#         for track in self.tracks:
#             yield track
