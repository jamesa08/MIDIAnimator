import * as Nodes from ".";

const nodeTypes = {
    midi_file_node: Nodes.MIDIFileNode,
    midi_track_node: Nodes.MIDITrackNode,
};

// json file for node types
// includes name, description, react_flow_id and component element
const NODE_TYPES = {
    midi_file_node: {
        name: "Get MIDI File",
        description: "Gets a MIDI provided a path to the file with the file picker.",
        react_component: Nodes.MIDIFileNode,
    },
    midi_track_node: {
        name: "Get MIDI Track",
        description: "Gets a MIDI track from an array of MIDI Tracks.",
        react_component: Nodes.MIDITrackNode,
    },
};

export default nodeTypes;
