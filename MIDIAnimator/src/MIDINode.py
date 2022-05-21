from . MIDIStructure import MIDITrack
from .. libs import mido
from .. utils.functions import gmProgramToName, _closestTempo
from .. utils.animation import *
from . Note import Note, PlayedNote
from typing import Dict, List, Tuple


class MIDINode:

    # maps the number to corresponding Note
    _noteDict: Dict[int, Note]
    _playedNoteList: List[PlayedNote]
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
        self._makePlayedNoteTimeline

        self._instrumnets = []
        

    def addInstrument(self, type: str, trackName: str, objectCollection):
        """add an instrument definition

        :param str type: type of animation. Choose between TODO: TBD
        Custom allows you to write your own animation driver code. See the docs for more info
        :param str trackName: name of the track in the MIDI file. Names of instruments in MIDI file _must_ be different.
        :param bpy.data.collection objectCollection: the object collection to be animated. Must be a `bpy collection`.
        """
        # this will initialize an instrument
        # objs = objectCollection.all_objects

        # for obj in objs:
        #     print(obj)


        track = self.findTrack(trackName)

        if track is None:
            raise RuntimeError(f"Track {trackName} does not exist.")

        self._instrumnets.append((type, track, objectCollection))

        # print(type, trackName, objectCollection)


    def getMIDIData(self):
        return self._tracks

    def _parseMIDI(self, file: str):
        """helper method that takes a MIDI file (type 0 and 1) and returns a list of MIDITracks 
        
        :param midFile: MIDI file
        :return: list of MIDITracks 
        """

        midiFile = mido.MidiFile(file)

        # make sure the file is not type 2
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

            # type 0 and 1
            if track.name:
                curTrack.name = track.name
            
            for msg in track:

                type = msg.type

                time += mido.tick2second(msg.time, midiFile.ticks_per_beat, tempo if midiFile.type == 0 else _closestTempo(tempoMap, time)[1])

                # channel messages
                if midiFile.type == 0 and not msg.is_meta and msg.type != "sysex":
                    # update tracks as they are read in
                    curChannel = msg.channel
                    curTrack = midiTracks[curChannel]
                
                # velocity 0 note_on messages need to be note_off
                if type == "note_on" and msg.velocity <= 0:
                    type = "note_off"

                if type == "note_on":
                    curTrack.addNoteOn(msg.channel, msg.note, msg.velocity, time)
                
                elif type == "note_off":
                    curTrack.addNoteOff(msg.channel, msg.note, msg.velocity, time)
                
                elif type == "program_change":
                    # General MIDI name
                    gmName = gmProgramToName(msg.program) if msg.channel != 9 else "Drumset"

                    if curTrack.name == "":
                        curTrack.name = gmName
                
                elif type == "control_change":
                    curTrack.addControlChange(msg.control, msg.channel, msg.value, time)
                
                elif type == "pitchwheel":
                    curTrack.addPitchwheel(msg.channel, msg.pitch, time)
                
                elif type == "aftertouch":
                    curTrack.addAftertouch(msg.channel, msg.value, time)

                # add track to tracks for type 1
                if midiFile.type == 1 and msg.is_meta and type == "end_of_track" and not curTrack._isEmpty():
                    midiTracks.append(curTrack)

            # make sure lists are of equal length
            assert curTrack._checkIfEqual(), "NoteOn's and NoteOff's are unbalanced! Please open an issuse on GitHub."

        # remove empty tracks        
        midiTracks = list(filter(lambda x: not x._isEmpty(), midiTracks))

        return midiTracks

    def _animate(self, instrument: Tuple[str, MIDITrack, bpy.types.Collection]) -> List[float]:
        # need a way to iterate over objects and then animate them
        scene = bpy.context.scene
        frameRate = getExactFps()

        n = 0
        animatedList = [0.0] * 128
        while n < len(self._noteDict):
            pass

    def run(self):
        pass

    def firstNoteTqime(self) -> float:
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


    def _makePlayedNoteTimeline(self):
        # iterate over List[MIDITrack]
        # make List[PlayedNote] sorted by start time for the Note

        # go through MidiTrack and the first time you come across a note number, make the Note and add to _noteDict

        # second pass and create list of PlayedNote from MidiTrack 
        # sort that list by startTime

        for track in self._tracks:
            for note in track.noteOn:
                if note[1] in self._noteDict:
                    # in dict
                    self._noteDict[note[1]].append(note[-1])
                else:
                    # not in dict
                    self._noteDict[note[1]] = [note[-1]]
        

    def __str__(self):
        pass
        # generate a multiline string with each line corresponding to item in List[PlayedNote]