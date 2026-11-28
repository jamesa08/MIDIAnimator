import { useEffect } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { useStateContext } from "../contexts/StateContext";
import { useSetInputs } from "../utils/graphOps";
import { useGroupContext } from "../contexts/GroupContext";
import { withSelected } from "./keyframes_from_object";
import NoteRollPreview from "../components/graph/NoteRollPreview";
import { TILE_PICTURE } from "../components/nodegraph/simplified";

function get_midi_track_data({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const { scopeId } = useGroupContext();
    const nodeData = useNodeSpec("get_midi_track_data");
    const { backEndState: state } = useStateContext();

    // load the node's spec (name, handles) once

    // get everything from the state and node data directly, so values set by the backend (e.g. over MCP) are kept
    // a mistyped connection can hand us a non-array, so don't trust the shape
    const tracks = state?.executed_inputs?.[id]?.["tracks"];
    const trackNames: string[] = Array.isArray(tracks) ? tracks.map((track: any) => track?.name) : [];
    // the track name that is currently set on the node
    const selectedTrackName: string = data.inputs?.track_name ?? "";

    // a track is picked automatically only while none is, not an undo step. a pick the file doesn't have is kept (it's
    // listed below), undo can put back the file it's from. not in a group's graph: the tracks are from one group node's
    // run, the graph is every group node's. the names are joined in the deps so this only re-runs when the track list
    // actually changes
    useEffect(() => {
        if (scopeId === null && !selectedTrackName && trackNames.length > 0) {
            setInputs(id, { track_name: trackNames[0] }, { commitToHistory: false });
        }
    }, [trackNames.join("\n"), selectedTrackName, scopeId, setInputs]);

    // a pick the file doesn't have shows as it is, nothing picked shows empty, so picking any track changes it
    const trackChoices = withSelected(
        trackNames.map((track) => ({ value: track, label: track })),
        selectedTrackName
    );

    const trackNameComponent = (
        <>
            <select className="node-field nodrag nopan" value={selectedTrackName} onChange={(event) => setInputs(id, { track_name: event.target.value })}>
                {trackChoices.length > 0 ? (
                    <>
                        {!selectedTrackName && <option value="" />}
                        {trackChoices.map((track, index) => (
                            <option key={index} value={track.value}>
                                {track.label}
                            </option>
                        ))}
                    </>
                ) : (
                    <option value={selectedTrackName}>No track names found</option>
                )}
            </select>
        </>
    );

    const uiInject = {
        track_name: trackNameComponent,
    };

    const hiddenHandles = {
        track_name: true,
    };

    // its tile in the simplified view is a piano roll of the notes from the last run
    const notes = state?.executed_results?.[id]?.notes;
    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} executor={execute} data={data} tile={<NoteRollPreview notes={notes} height={TILE_PICTURE} />} />;
}

// takes in a hashmap and returns a hashmap of the output keys filled
function execute(input: any): any {
    console.log("executing");
    return {
        tracks: [],
    };
}

export default get_midi_track_data;
