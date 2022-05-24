from .. utils.functions import removeDuplicates, gmProgramToName, _closestTempo
from .. libs import mido
from typing import List, Tuple, Dict

class MIDINote:
    # number
    # structure (channel, note, velocity, time)

    channel: int
    noteNumber: int
    velocity: int
    timeOn: float
    timeOff: float  # ignore to start

    def __init__(self, channel: int, noteNumber: int, velocity: int, timeOn: float, timeOff: float):
        channel = self.channel
        noteNumber = self.number
        velocity= self.velocity
        timeOn = self.timeOn
        timeOff = self.timeOff


class MIDITrack:
    # make MIDINote object?
    name: str

    # structure (channel, note, velocity, time)

    _notesCombined: List[MIDINote]

    notesOn: List[Tuple[int, int, int, float]]
    notesOff: List[Tuple[int, int, int, float]]
    
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
        self.notesOn = []

        # note off event
        # structure (channel, note, velocity, time)
        self.notesOff = []

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
        self.notesOn.append((channel, noteNumber, velocity, timeOn))

    def addNoteOff(self, channel: int, noteNumber: int, velocity: int, timeOff: float) -> None:
        """adds a Note Off event

        :param int channel: MIDI channel
        :param int noteNumber: the note number, TODO range
        :param int velocity: the note velocity, TODO range
        :param float timeOff: the note time off, in seconds
        """
        self.notesOff.append((channel, noteNumber, velocity, timeOff))

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

    def combineNoteOnOffLists(self):
        activeNotes = []
        noteOnIndex = 0
        noteOffIndex = 0
        while noteOnIndex < len(self.notesOn):
            pass

    def postProcess(self) -> None:
        # build up _notesToPlay list of MIDINote() objects and store in an instance var
        # sorted by timeOn (auto)
        # clear out old lists using .clear()
        
        for channel, noteNumber, velocity, timeOn in self.notesOn:
            # TODO: algorithim to match timeOn and timeOff
            self._notesCombined.append(MIDINote(channel, noteNumber, velocity, timeOn, timeOff=0))
        
        instanceVars = (self.notesOn, self.notesOff)


        for instance in instanceVars:
            instance.clear()



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

class MIDIFile:
    _tracks = List[MIDITrack]

    def __init__(self, midiFile: str):
        """
        open file and store it as data in lists
        tracks with channels and track names, timesOn and off information
        for each track, velocity and MIDI CC info for each track, etc

        :param str midiFile: MIDI file path
        """
        self._noteDict = {}

        # store lists of info
        self._tracks = self._parseMIDI(midiFile)

    def getMIDIData(self):
        return self._tracks

    def _parseMIDI(self, file: str):
        """helper method that takes a MIDI file (instrumentType 0 and 1) and returns a list of MIDITracks

        :param midFile: MIDI file
        :return: list of MIDITracks
        """

        midiFile = mido.MidiFile(file)

        # make sure the file is not instrumentType 2
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
                if msg.midiType == "set_tempo":
                    tempo = msg.tempo
                    tempoMap.append((time, msg.tempo))

        # iterate over file
        for track in midiFile.tracks:
            time = 0
            tempo = 500000

            if midiFile.type == 0:
                curChannel = 0
                curTrack = midiTracks[curChannel]
            else:
                curTrack = MIDITrack("")

            # instrumentType 0 and 1
            if track.name:
                curTrack.name = track.name

            for msg in track:

                curType = msg.midiType

                time += mido.tick2second(msg.time, midiFile.ticks_per_beat,
                                         tempo if midiFile.type == 0 else _closestTempo(tempoMap, time)[1])

                # channel messages
                if midiFile.type == 0 and not msg.is_meta and msg.midiType != "sysex":
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

                    if curTrack.name == "":
                        curTrack.name = gmName

                elif curType == "control_change":
                    curTrack.addControlChange(msg.control, msg.channel, msg.value, time)

                elif curType == "pitchwheel":
                    curTrack.addPitchwheel(msg.channel, msg.pitch, time)

                elif curType == "aftertouch":
                    curTrack.addAftertouch(msg.channel, msg.value, time)

                # add track to tracks for instrumentType 1
                if midiFile.type == 1 and msg.is_meta and curType == "end_of_track" and not curTrack._isEmpty():
                    midiTracks.append(curTrack)

            # make sure lists are of equal length
            assert curTrack._checkIfEqual(), "NoteOn's and NoteOff's are unbalanced! Please open an issuse on GitHub."

        # remove empty tracks
        midiTracks = list(filter(lambda x: not x._isEmpty(), midiTracks))

        # for each track, call track.postProcess()

        return midiTracks

    def firstNoteTime(self) -> float:
        """finds the smallest noteOn message

        :return float: the smallest noteOn message
        """
        return min([min(track.noteOn, key=lambda x: x[-1])[-1] for track in self._tracks])

    def lastNoteTime(self) -> float:
        """finds the largest last noteOff message of all tracks

        :return float: the largest noteOff message
        """
        return max([max(track.noteOff, key=lambda x: x[-1])[-1] for track in self._tracks])

    def findTrack(self, name) -> MIDITrack:
        """Finds the track with a name

        :param str name: The name of the track to be returned
        :return list: The track with the specified name
        """
        for x in self._tracks:
            if x.name == name:
                return x

    # TODO: remove this vvv
    # def _makePlayedNoteTimeline(self):
    #     # iterate over List[MIDITrack]
    #     # make List[PlayedNote] sorted by start time for the Note
    #
    #     # go through MidiTrack and the first time you come across a note number, make the Note and add to _noteDict
    #
    #     # second pass and create list of PlayedNote from MidiTrack
    #     # sort that list by startTime
    #
    #     for track in self._tracks:
    #         for note in track.noteOn:
    #             if note[1] in self._noteDict:
    #                 # in dict
    #                 self._noteDict[note[1]].append(note[-1])
    #             else:
    #                 # not in dict
    #                 self._noteDict[note[1]] = [note[-1]]

    def __str__(self):
        out = []
        for track in self.tracks:
            out.append(str(track))

        return "\n".join(out)

