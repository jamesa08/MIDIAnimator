from . MIDIStructure import MIDIInstrument, MIDITrack
from .. libs import mido
from .. utils.functions import gmProgramToName

class MIDINode:
    def __init__(self, midiFile: str):
        """
        open file and store it as data in lists
        tracks with channels and track names, timesOn and off information
        for each track, velocity and MIDI CC info for each track, etc

        :param str midiFile: MIDI file path
        """

        # store lists of info
        self.instruments = self._parseMIDI(midiFile)

    def addInstrument(self, type: str, instrumentName: str, objectCollection: str):
        """add an instrument definition

        :param str type: type of animation. Choose between Bounce, Hi-Hat, Drumstick, or custom. 
        Custom allows you to write your own animation driver code. See the docs for more info
        :param str instrumentName: name of the instrument track. Names of instruments in MIDI file _must_ be different.
        :param str objectCollection: name of the object collection. 
        Objects in the collection should be structured as follows; 
        name_noteNumberOrNoteName (e.g., SnareDrum_D1 or SnareDrum_26)
        """
        # this will initialize an instrument
        pass

    def getMIDIData(self):
        return self.instruments

    def _parseMIDI(self, midiFile: str):
        """helper method that takes a MIDI file and returns a MIDIINstrument object
        
        :param midFile: MIDI file
        :return: MIDIINstrument object
        """
        # TODO: add type 1
        assert midiFile.type == 0

        # Type 0 tracks depend on MIDI Channels for the different instruments
        # Instance in 16 MIDI instruments
        if midiFile.type == 0:
            midiInstruments = [MIDIInstrument("") for _ in range(16)]
            midiTracks = [ins.addTrack(MIDITrack("")) for ins in midiInstruments]

        for track in midiFile.tracks:
            time = 0
            tempo = 500000
            curChannel = 0

            # set inital track to the first
            curTrack = midiTracks[curChannel]

            for msg in track:
                type = msg.type

                # channel messages
                if not msg.is_meta and msg.type != "sysex":
                    # update tracks as they are read in
                    curTrack = midiTracks[msg.channel]
                    curIns = midiInstruments[curChannel]
                    curChannel = msg.channel
                
                # tempo message
                if msg.type == "set_tempo":
                    tempo = msg.tempo

                # MIDIInstrument name 
                if msg.is_meta and msg.type == "instrument_name":
                    if curIns.name == "":
                        curIns.name = msg.name
                
                # MIDITrack name
                if msg.is_meta and msg.type == "track_name":
                    # since this is a type 0 track, either use program changes
                    # or make all the track names the same name
                    curTrack.name = msg.name

                # velocity 0 note_on messages need to be note_off
                if type == "note_on" and msg.velocity <= 0:
                    type = "note_off"

                time += mido.tick2second(msg.time, midiFile.ticks_per_beat, tempo)

                if type == "note_on":
                    curTrack.addNoteOn(msg.channel, msg.note, msg.velocity, time)
                
                elif type == "note_off":
                    curTrack.addNoteOff(msg.channel, msg.note, msg.velocity, time)
                
                elif type == "program_change":
                    # General MIDI name
                    gmName = gmProgramToName(msg.program) if msg.channel != 9 else "Drumset"
                    
                    if curIns.name == "":
                        curIns.name = gmName
                    
                    if curTrack.name == "":
                        curTrack.name = gmName
                
                elif type == "control_change":
                    curTrack.addControlChange(msg.control, msg.channel, msg.value, time)
                
                elif type == "pitchwheel":
                    curTrack.addPitchwheel(msg.channel, msg.pitch, time)
                
                elif type == "aftertouch":
                    curTrack.addAftertouch(msg.channel, msg.value, time)

            # make sure lists are of equal length
            assert curTrack._checkIfEqual(), "NoteOn's and NoteOff's are unbalanced! Please open an issuse on GitHub."


        # remove empty tracks
        for i, ins in enumerate(midiInstruments):
            if ins._isEmpty():
                midiInstruments[i] = None

        return midiInstruments