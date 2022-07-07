# run in the MIDIAnimator/ directory
# python3 tests/read_file.py
# modules will import correctly this way

import sys
sys.path.append(".")

from MIDIAnimator.src.MIDINode import MIDINode
from MIDIAnimator.data_structures.midi import MIDITrack
from MIDIAnimator.libs import mido
from MIDIAnimator.utils.functions import gmProgramToName, _closestTempo


# overwrite for now, utils is backup
def _closestTempo(vals, n, sortList=False):
    if sortList:
        vals.sort()
    
    if len(vals) == 1:
        return vals[0]
    
    for first, second in zip(vals, vals[1:] + [(0, float('inf'))]):
        if first[0] <= n < second[0]:
            return first
    
    return second


def main():
    m0 = "../MIDIFiles/midijam/smw_donut_polka.mid"
    m1 = "../MIDIFiles/Yamaha-XG-Midi-Library/PSR SYNTH/PSR-500/PSR-500Demo/Clean Guitar 2.S087.MID"
    m2 = "../MIDIAnimator/tests/type_0_mangled.mid"
    m3 = "../MIDIFiles/Professional MIDI files collection/ROLAND/Demos/JV-1010/demo001.mid"
    m4 = "../MIDIFiles/testMidi/Type_1_20_ins.mid"
    m5 = "../MIDIFiles/testMidi/Type_1_test.mid"
    m6 = "../MIDIFiles/testMidi/type_1_tempo.mid"
    m7 = "../MIDIFiles/testMidi/Type_1_20_ins_tempo.mid"

    midiFile = mido.MidiFile(m1)

    if midiFile.type == 0:
        # Type 0 tracks depend on MIDI Channels for the different tracks
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

    if midiFile.type == 0:
        for i, track in enumerate(midiTracks):
            if track._isEmpty():
                midiTracks.remove(track)

    # if midiFile.type == 0:
    #     # Type 0 tracks depend on MIDI Channels for the different instruments
    #     # Instance in 16 MIDI instruments & link tracks to the instruments
    #     midiTracks = [MIDITrack("") for _ in range(16)]


    #     for track in midiFile.tracks:
    #         time = 0
    #         tempo = 500000
    #         curChannel = 0

    #         # set inital track to the first
    #         curTrack = midiTracks[curChannel]

    #         for msg in track:
    #             type = msg.type

    #             # channel messages
    #             if not msg.is_meta and msg.type != "sysex":
    #                 # update tracks as they are read in
    #                 curChannel = msg.channel
    #                 curTrack = midiTracks[curChannel]

    #             # tempo message
    #             if msg.type == "set_tempo":
    #                 tempo = msg.tempo
                
    #             # MIDITrack name
    #             if msg.is_meta and msg.type == "track_name":
    #                 # since this is a type 0 track, either use program changes
    #                 # or make all the track names the same name
    #                 curTrack.name = msg.name

    #             # velocity 0 note_on messages need to be note_off
    #             if type == "note_on" and msg.velocity <= 0:
    #                 type = "note_off"

    #             time += mido.tick2second(msg.time, midiFile.ticks_per_beat, tempo)

    #             if type == "note_on":
    #                 curTrack.addNoteOn(msg.channel, msg.note, msg.velocity, time)
                
    #             elif type == "note_off":
    #                 curTrack.addNoteOff(msg.channel, msg.note, msg.velocity, time)
                
    #             elif type == "program_change":
    #                 # General MIDI name
    #                 gmName = gmProgramToName(msg.program) if msg.channel != 9 else "Drumset"
                    
    #                 # if curIns.name == "":
    #                 #     curIns.name = gmName
                    
    #                 if curTrack.name == "":
    #                     curTrack.name = gmName
                
    #             elif type == "control_change":
    #                 curTrack.addControlChange(msg.control, msg.channel, msg.value, time)
                
    #             elif type == "pitchwheel":
    #                 curTrack.addPitchwheel(msg.channel, msg.pitch, time)
                
    #             elif type == "aftertouch":
    #                 curTrack.addAftertouch(msg.channel, msg.value, time)

    #         # make sure lists are of equal length
    #         assert curTrack._checkIfEqual(), "NoteOn's and NoteOff's are unbalanced! Please open an issuse on GitHub."


    #     # for i, ins in enumerate(midiInstruments):
    #     #     if ins._isEmpty():
    #     #         midiInstruments[i] = None



    #     for i, track in enumerate(midiTracks):
    #         if track._isEmpty():
    #             midiTracks.remove(track)
        
    # elif midiFile.type == 1:
    #     tempoMap = []
    #     midiTracks = []

    #     # get tempo map first
    #     time = 0
    #     tempo = 0
    #     for msg in mido.merge_tracks(midiFile.tracks):
    #         time += mido.tick2second(msg.time, midiFile.ticks_per_beat, tempo)
    #         if msg.type == "set_tempo":
    #             tempo = msg.tempo
    #             tempoMap.append((time, msg.tempo))


    #     # iterate over file
    #     for track in midiFile.tracks:
    #         time = 0
    #         curTrack = MIDITrack("")
    #         tempo = 500000

    #         if track.name:
    #             curTrack.name = track.name
            
    #         for msg in track:
    #             type = msg.type

    #             time += mido.tick2second(msg.time, midiFile.ticks_per_beat, closestTempo(tempoMap, time)[1])

    #             # velocity 0 note_on messages need to be note_off
    #             if type == "note_on" and msg.velocity <= 0:
    #                 type = "note_off"

    #             if type == "note_on":
    #                 curTrack.addNoteOn(msg.channel, msg.note, msg.velocity, time)
                
    #             elif type == "note_off":
    #                 curTrack.addNoteOff(msg.channel, msg.note, msg.velocity, time)
                
    #             elif type == "program_change":
    #                 # General MIDI name
    #                 gmName = gmProgramToName(msg.program) if msg.channel != 9 else "Drumset"

    #                 if curTrack.name == "":
    #                     curTrack.name = gmName
                
    #             elif type == "control_change":
    #                 curTrack.addControlChange(msg.control, msg.channel, msg.value, time)
                
    #             elif type == "pitchwheel":
    #                 curTrack.addPitchwheel(msg.channel, msg.pitch, time)
                
    #             elif type == "aftertouch":
    #                 curTrack.addAftertouch(msg.channel, msg.value, time)


    #             # add track to tracks
    #             if msg.is_meta and type == "end_of_track" and not curTrack._isEmpty():
    #                 midiTracks.append(curTrack)

    # else:
    #     raise
    
    print("\n\n")
    print("-"*80)
    print("\n\n")
    
    for i, track in enumerate(midiTracks):
        if track is not None:
            print(f"{i}: {track.name}, {len(track.notesOn)=}")
            print(track)

    # for i, track in enumerate(midiTracks):
    #     print(f"{i}: {track.name}, notes={len(track.noteOn)}")

main()
