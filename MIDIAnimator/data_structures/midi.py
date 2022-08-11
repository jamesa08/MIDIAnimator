from __future__ import annotations
from .. utils import removeDuplicates, gmProgramToName, _closestTempo
from .. libs import mido
from dataclasses import dataclass
from typing import List, Tuple, Dict
from bpy.path import abspath

@dataclass
class MIDINote:
    channel: int
    noteNumber: int
    velocity: int
    timeOn: float
    timeOff: float
    
    def __lt__(self, other):
        return self.timeOn < other.timeOn

@dataclass
class MIDIEvent:
    channel: int
    value: float
    time: float

    def __lt__(self, other):
        return self.timeOn < other.timeOn

class MIDITrack:
    name: str
    
    controlChange: Dict[int, List[MIDIEvent]]
    pitchwheel: List[MIDIEvent]
    aftertouch: List[MIDIEvent]

    _noteTable: Dict[Tuple[int, int], List[MIDINote]]

    def __init__(self, name: str):
        """initialize a MIDITrack

        :param str name: name of track
        """
        self.name = name

        self.notes = []
        self.controlChange = dict()
        self.pitchwheel = []
        self.aftertouch = []

        self._noteTable = dict()

    def addNoteOn(self, channel: int, noteNumber: int, velocity: int, timeOn: float) -> None:
        """adds a Note Event

        :param int channel: the MIDI Channel the note is on
        :param int noteNumber: the note number, range from 0-127
        :param int velocity: the note velocity, range 0-127
        :param float timeOn: the note time on, in seconds
        """
        key = (channel, noteNumber)
        note = MIDINote(channel, noteNumber, velocity, timeOn, timeOff=-1.0)

        if key in self._noteTable:
            self._noteTable[key].append(note)
        else:
            self._noteTable[key] = [note]
        
        self.notes.append(note)

    def addNoteOff(self, channel: int, noteNumber: int, velocity: int, timeOff: float) -> None:
        """adds a Note Off event

        :param int channel: MIDI channel
        :param int noteNumber: the note number, TODO range
        :param int velocity: the note velocity, TODO range
        :param float timeOff: the note time off, in seconds
        """

        try:
            # find matching note on message
            key = (channel, noteNumber)
            note = self._noteTable[key]

            # assume the first note on message for this note is the one that matches with this note off
            note[0].timeOff = timeOff

            # remove this note for this note number b/c we have the note off for this note
            del note[0]
        except IndexError:
            raise RuntimeError("NoteOff message has no NoteOn message! Please open an issue on GitHub.")            

    def addControlChange(self, control_number: int, channel: int, value: int, time: float):
        """add a control change value
        automatically checks if number has been added

        :param int control_number: the control change number
        :param int channel: MIDI channel number
        :param int value: value of the control change
        :param float time: time value (in seconds)
        """

        event = MIDIEvent(channel, value, time)

        if control_number in self.controlChange:
            # in dict
            self.controlChange[control_number].append(event)
        else:
            # not in dict
            self.controlChange[control_number] = [event]

    def addPitchwheel(self, channel: int, value: float, time: float) -> None:
        """add a pitchwheel event

        :param int channel: the MIDI channel number
        :param float value: value of the pitch wheel TODO range
        :param float time: time value (in seconds)
        """
        self.pitchwheel.append(MIDIEvent(channel, value, time))

    def addAftertouch(self, channel: int, value: float, time: float) -> None:
        """add a aftertouch event

        :param int channel: the MIDI channel number
        :param float value: value of the aftertouch, TODO range
        :param float time: time value (in seconds)
        """
        self.aftertouch.append(MIDIEvent(channel, value, time))

    def _isEmpty(self) -> bool:
        """checks if MIDITrack is empty

        :return bool: returns True if MIDITrack is empty, False otherwise
        """

        variables = [self.notes, self.controlChange, self.pitchwheel, self.aftertouch]

        return len(variables) == sum([len(v) == 0 for v in variables])

    def allUsedNotes(self) -> list:
        """Returns a list of all used notes in a MIDI Track.

        :return list: a list of all used notes
        """

        return removeDuplicates([note.noteNumber for note in self.notes])

    def __str__(self) -> str:
        # TODO: Refactor & optimize
        # Not Pythonic!

        out = [f"MIDITrack(name'{self.name}', \nnotes=[\n\t"]

        for i, note in enumerate(self.notes):
            out.append(str(note))
            if i != len(self.notes) - 1:
                out.append(",\n\t")
        
        out.append("],\ncontrolChanges={\n\t")

        for i, key in enumerate(self.controlChange):
            out.append(f"control_number={key}: ")
            for j, event in enumerate(self.controlChange[key]):
                out.append(str(event))
                
                if j != len(self.controlChange[key]) - 1:
                    out.append(",\n\t\t")
        
            if i != len(self.controlChange) - 1:
                out.append(",\n\t")

        out.append("},\nafterTouch=[\n\t")

        for i, event in enumerate(self.aftertouch):
            out.append(str(event))
            if i != len(self.aftertouch) - 1:
                out.append(",\n\t")

        out.append("],\npitchwheel=[\n\t")

        for i, event in enumerate(self.pitchwheel):
            out.append(str(event))
            if i != len(self.pitchwheel) - 1:
                out.append(",\n\t")

        out.append("]\n)")
        
        return "".join(out)

    def __add__(self, other) -> MIDITrack:
        print(f"INFO: Attempting to merge tracks '{self.name}' & '{other.name}' ...")
        addedTrack = MIDITrack(f"{self.name} & {other.name}")

        addedTrack.notes = sorted(self.notes + other.notes)
        
        controlChangeCloned = self.controlChange.copy()
        controlChangeCloned.update(other.controlChange)

        addedTrack.controlChange = controlChangeCloned
        
        addedTrack.pitchweel = sorted(self.pitchwheel + other.pitchwheel)
        addedTrack.aftertouch = sorted(self.aftertouch + other.aftertouch)
        
        return addedTrack

    def __repr__(self) -> str:
        type_ = type(self)
        module = type_.__module__
        qualname = type_.__qualname__

        return f"<{module}.{qualname} object \"{self.name}\", at {hex(id(self))}>"

class MIDIFile:
    _tracks = List[MIDITrack]

    def __init__(self, midiFile: str):
        """
        open file and store it as data in lists
        tracks with channels and track names, timesOn and off information
        for each track, velocity and MIDI CC info for each track, etc

        :param str midiFile: MIDI file path
        """

        # store lists of info
        self._tracks = self._parseMIDI(midiFile)
        

    def getMIDITracks(self) -> List[MIDITrack]:
        return self._tracks

    def _parseMIDI(self, file: str) -> List[MIDITrack]:
        """helper method that takes a MIDI file (instrumentType 0 and 1) and returns a list of MIDITracks

        :param midFile: MIDI file
        :return: list of MIDITracks
        """

        midiFile = mido.MidiFile(abspath(file))

        assert midiFile.type in range(2), "Type 2 MIDI Files are not supported!"

        if midiFile.type == 0:
            # Type 0
            # Tracks depend on MIDI Channels for the different tracks
            # Instance in 16 MIDI tracks
            midiTracks = [MIDITrack("") for _ in range(16)]
        else:
            # Type 1
            tempoMap = []
            midiTracks = []

            time = 0
            tempo = 0

            # get tempo map first
            for msg in mido.merge_tracks(midiFile.tracks):
                time += mido.tick2second(msg.time, midiFile.ticks_per_beat, tempo)
                if msg.type == "set_tempo":
                    tempoMap.append((time, msg.tempo))


        for track in midiFile.tracks:
            time = 0
            tempo = 500000

            if midiFile.type == 0:
                curChannel = 0
                curTrack = midiTracks[curChannel]
            else:
                curTrack = MIDITrack("")

            if track.name:
                curTrack.name = track.name


            for msg in track:
                curType = msg.type

                if midiFile.type == 0 and msg.type == "set_tempo":
                    tempo = msg.tempo

                time += mido.tick2second(msg.time, midiFile.ticks_per_beat,
                                         tempo if midiFile.type == 0 else _closestTempo(tempoMap, time)[1])

                # channel messages
                if midiFile.type == 0 and not msg.is_meta and msg.type != "sysex":
                    # update tracks as they are read in
                    curChannel = msg.channel
                    curTrack = midiTracks[curChannel]

                # velocity 0 note_on messages need to be note_off
                if curType == "note_on" and msg.velocity <= 0:
                    curType = "note_off"

                if curType == "note_on":
                    curTrack.addNoteOn(msg.channel, msg.note, msg.velocity, time)

                elif curType == "note_off":
                    curTrack.addNoteOff(msg.channel, msg.note, msg.velocity, time)

                elif curType == "program_change":
                    # General MIDI name
                    gmName = gmProgramToName(msg.program) if msg.channel != 9 else "Drumset"

                    if curTrack.name == "" or (midiFile.type == 0 and curTrack.name == f"Track {curChannel + 1}"):
                        curTrack.name = gmName
                    
                elif curType == "control_change":
                    curTrack.addControlChange(msg.control, msg.channel, msg.value, time)

                elif curType == "pitchwheel":
                    curTrack.addPitchwheel(msg.channel, msg.pitch, time)

                elif curType == "aftertouch":
                    curTrack.addAftertouch(msg.channel, msg.value, time)
                
                if midiFile.type == 0 and curTrack.name == "":
                    curTrack.name = f"Track {curChannel + 1}"

                # add track to tracks for instrumentType 1
                if midiFile.type == 1 and msg.is_meta and curType == "end_of_track" and not curTrack._isEmpty():
                    midiTracks.append(curTrack)

        # remove empty tracks
        midiTracks = list(filter(lambda track: not track._isEmpty(), midiTracks))

        # make sure notes are sorted
        # & delete noteTable (not needed)
        for track in midiTracks:
            track.notes.sort()
            del track._noteTable


        return midiTracks
    
    def findTrack(self, name) -> MIDITrack:
        """Finds the track with a specified name

        :param str name: The name of the track to be returned
        :return list: The track with the specified name
        """
        for track in self._tracks:
            if track.name == name:
                return track
    
    def listTrackNames(self) -> List[str]:
        return [str(track.name) for track in self._tracks]
    
    def mergeTracks(self, track1: MIDITrack, track2: MIDITrack, name=None):
        merged = track1 + track2
        if name: merged.name = name
        return merged

    def __str__(self):
        out = []
        for track in self._tracks:
            out.append(str(track))

        return "\n".join(out)

    def __iter__(self):
        for track in self._tracks:
            yield track