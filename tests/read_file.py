# run in the MIDIAnimator/ directory
# python3 tests/read_file.py
# modules will import correctly this way

import sys
sys.path.append(".")

from MIDIAnimator.src.MIDINode import MIDINode
from MIDIAnimator.src.MIDIStructure import MIDIInstrument, MIDITrack
from MIDIAnimator.libs import mido
from MIDIAnimator.utils.functions import gmProgramToName

def main():
    m0 = "./midijam/smw_donut_polka.mid"
    m1= "../MIDIFiles/Yamaha-XG-Midi-Library/PSR SYNTH/PSR-500/PSR-500Demo/Clean Guitar 2.S087.MID"
    m2 = "../MIDIAnimator/tests/type_0_mangled.mid"
    midiFile = mido.MidiFile(m1)

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

    # -------------------------------------------------------
    for i, ins in enumerate(midiInstruments):
        if ins._isEmpty():
            midiInstruments[i] = None

    for chl, ins in enumerate(midiInstruments):
        if ins is not None:
            print("channel: " + str(chl+1), repr(ins))
            for trk in ins:
                print(trk)

main()
