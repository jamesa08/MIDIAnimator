from . MIDIStructure import MIDITrack, BlenderTrack
from .. libs import mido
from .. utils.functions import gmProgramToName, _closestTempo
from .. utils.animation import *
from . Note import Note, PlayedNote
from typing import Dict, List, Tuple
import bpy

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


class MIDIFile:

    # maps the number to corresponding Note
    # _noteDict: Dict[int, Note]
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
        self._makePlayedNoteTimeline()

        self._instruments = []


    def getMIDIData(self):
        return self._tracks

    def _parseMIDI(self, file: str):
        """helper method that takes a MIDI file (instrumentType 0 and 1) and returns a list of MIDITracks

        :param midFile: MIDI file
        :return: list of MIDITracks
        """

        midiFile = mido.MidiFile(file)

        # make sure the file is not instrumentType 2
        assert midiFile.midiType in range(2), "Type 2 MIDI Files are not supported!"

        if midiFile.midiType == 0:
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

            if midiFile.midiType == 0:
                curChannel = 0
                curTrack = midiTracks[curChannel]
            else:
                curTrack = MIDITrack("")

            # instrumentType 0 and 1
            if track.name:
                curTrack.name = track.name

            for msg in track:

                curType = msg.midiType

                time += mido.tick2second(msg.time, midiFile.ticks_per_beat, tempo if midiFile.midiType == 0 else _closestTempo(tempoMap, time)[1])

                # channel messages
                if midiFile.midiType == 0 and not msg.is_meta and msg.midiType != "sysex":
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
                if midiFile.midiType == 1 and msg.is_meta and curType == "end_of_track" and not curTrack._isEmpty():
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