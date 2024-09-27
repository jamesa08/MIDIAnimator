import * as Nodes from ".";

// linking node IDs from json to node components
const nodeTypes = {
    get_midi_file: Nodes.GetMIDIFileNode,
    get_midi_track_data: Nodes.GetMIDITrackNode,
    viewer: Nodes.ViewerNode,
    scene_link: Nodes.SceneLink,
    keyframes_from_object: Nodes.KeyframesFromObjectNode,
};

export default nodeTypes;
