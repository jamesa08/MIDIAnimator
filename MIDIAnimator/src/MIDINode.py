from . MIDIStructure import MIDIInstrument, MIDITrack
from .. libs import mido

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

        instrument = MIDIInstrument("", 0)
        instruments = []
        oldInstrumentName = ""
        instrumentIndex = 0

        # 500000 ticks = 120 bpm
        currentTempo = 500000

        midoMidiFile = mido.MidiFile(midiFile)

        for i, midoTrack in enumerate(midoMidiFile.tracks):
            track = MIDITrack(midoTrack.name, i)

            delta = 0
            notMetaMsg = False

            for midoMsg in midoTrack:
                delta += midoMsg.time

                if not midoMsg.is_meta:
                    # if we hit a non meta message, store that
                    notMetaMsg = True

                if midoMsg.is_meta:
                    if midoMsg.type == "set_tempo":
                        currentTempo = midoMsg.tempo

                    if midoMsg.type == "instrument_name":
                        # the MIDIInstrument name
                        # check to see if we are on a new instrument track
                        if oldInstrumentName != midoMsg.name:
                            # store old instrumnet:
                            # only store the track if doesn't have just MetaMessages
                            # otherwise, just skip and don't store
                            if notMetaMsg:
                                instruments.append(instrument)
                            else:
                                instrumentIndex -= 1

                            # update instrument index
                            instrumentIndex += 1

                            # make new instrument
                            instrument = MIDIInstrument(
                                midoMsg.name, instrumentIndex)

                            # store new name as well
                            oldInstrumentName = midoMsg.name

                    # EOF fix
                    if (midoMsg.type == "end_of_track") and (i == len(midoMidiFile.tracks) - 1) and notMetaMsg:
                        instruments.append(instrument)

                if midoMsg.type == "note_on":
                    track.timeOn.append(mido.tick2second(
                        delta, midoMidiFile.ticks_per_beat, currentTempo))
                    track.noteNumber.append(midoMsg.note)
                    track.velocity.append(midoMsg.velocity)

                elif midoMsg.type == "note_off":
                    track.timeOff.append(mido.tick2second(
                        delta, midoMidiFile.ticks_per_beat, currentTempo))
                    # track.addNote(MIDINote(midoMsg.channel, midoMsg.note,
                    #                        tempTimeOn, mido.tick2second(currentTime, midoMidiFile.ticks_per_beat, currentTempo), midoMsg.velocity))
                elif midoMsg.type == "control_change":
                    track.addControlChange(midoMsg.control, midoMsg.channel, midoMsg.value, mido.tick2second(
                        delta, midoMidiFile.ticks_per_beat, currentTempo))
            
            assert track._checkIfEqual(), "lists are not equal!"
            instrument.addTrack(track)
        

        return instruments
