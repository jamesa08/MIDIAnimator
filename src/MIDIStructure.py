from .. utils.functions import noteToName, nameToNote

class MIDITrack:
    def __init__(self, name: str, index: int):
        """intialize a MIDITrack

        :param str name: name of track
        :param int index: position of track (not used, remove?)
        """
        self.name = name
        self.index = index
        self.timeOn = []
        self.timeOff = []
        self.noteNumber = []
        self.velocity = []

        # structure: dict(control_change_number: [(channel, value, time), ...])
        self.controlChange = {}
    def addTimeOn(self, time: float) -> None:
        """adds a time on value

        :param float time: time on value to add
        """
        self.timeOn.append(time)

    def addTimeOff(self, time: float) -> None:
        """adds a time off value

        :param float time: time off value to add
        """
        self.timeOff.append(time)

    def addNoteNumber(self, note: int) -> None:
        """adds a note number value

        :param int note: note number value to add
        """
        self.noteNumber.append(note)

    def addVelocity(self, velocity: int) -> None:
        """adds a velocity value

        :param int velocity: velocity value to add
        """
        self.velocity.append(velocity)

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

    def _checkIfEqual(self) -> bool:
        """checks if timesOn, timesOff and noteNumbers are of equal length

        :return bool: returns True if list are of equal length, False otherwise
        """
        return len(self.noteNumber) == len(self.timeOn) == len(self.timeOff) == len(self.velocity)

    def __str__(self) -> str:
        out = [f"MIDITrack(name='{self.name}', index={self.index}, notes=["]
        
        for i, (note, timeOn, timeOff, velocity) in enumerate(zip(self.noteNumber, self.timeOn, self.timeOff, self.velocity)):
            out.append(f"(noteNumber={note} timeOn={timeOn}, timeOff={timeOff}, velocity={velocity})")

            if i != len(self.timeOn) - 1:
                out.append(", ")

        out.append("], control_changes={")
        
        for i, key in enumerate(self.controlChange):
            out.append(f"control_number={key}: ")
            for j, (channel, value, time) in enumerate(self.controlChange[key]):
                out.append(f"(channel={channel}, value={value}, time={time})")
                
                if j != len(self.controlChange[key]) - 1:
                    out.append(", ")
        
            if i != len(self.controlChange) - 1:
                    out.append(", ")

        out.append("})")

        return "".join(out)

    def __iter__(self):
        for note in self.notes:
            yield note

class MIDIInstrument:
    def __init__(self, name: str, index: int):
        self.name = name
        self.index = index
        self.tracks = []
    
    def addTrack(self, track: MIDITrack) -> None:
        self.tracks.append(track)

    def addTracks(self, tracks: list) -> None:
        self.tracks = tracks
    
    def __str__(self) -> str:
        out = [f"MIDIInstrument(name='{self.name}', index={self.index}, tracks=["]
        
        for i, track in enumerate(self.tracks):
            out.append(str(track))

            if i != len(self.tracks) - 1:
                out.append(", ")

        out.append("])")

        return "".join(out)
    
    def __repr__(self) -> str:
        type_ = type(self)
        module = type_.__module__
        qualname = type_.__qualname__

        return f"<{module}.{qualname} object \"{self.name}\", index {self.index} at {hex(id(self))}>"

    def __iter__(self):
        for track in self.tracks:
            yield track


