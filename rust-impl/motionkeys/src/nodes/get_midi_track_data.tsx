import { useEffect } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/nodeEntries";
import { useStateContext } from "../contexts/StateContext";
import { useSetInputs } from "../utils/graphOps";

function get_midi_track_data({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const nodeData = useNodeSpec("get_midi_track_data");
    const { backEndState: state } = useStateContext();

    // load the node's spec (name, handles) once

    // get everything from the state and node data directly, so values set by the backend (e.g. over MCP) are kept
    // a mistyped connection can hand us a non-array, so don't trust the shape
    const tracks = state?.executed_inputs?.[id]?.["tracks"];
    const trackNames: string[] = Array.isArray(tracks) ? tracks.map((track: any) => track?.name) : [];
    // the track name that is currently set on the node
    const selectedTrackName: string = data.inputs?.track_name ?? "";

    // only pick a track automatically when none is set or the set one doesn't exist in the tracks, not an undo step.
    // the names are joined in the deps so this only re-runs when the track list actually changes
    useEffect(() => {
        if (trackNames.length > 0 && !trackNames.includes(selectedTrackName)) {
            setInputs(id, { track_name: trackNames[0] }, { commitToHistory: false });
        }
    }, [trackNames.join("\n"), selectedTrackName]);

    const trackNameComponent = (
        <>
            <select className="node-field nodrag nopan" value={selectedTrackName} onChange={(event) => setInputs(id, { track_name: event.target.value })}>
                {trackNames.length > 0 ? (
                    trackNames.map((track, index) => (
                        <option key={index} value={track}>
                            {track}
                        </option>
                    ))
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

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} executor={execute} data={data} />;
}

// takes in a hashmap and returns a hashmap of the output keys filled
function execute(input: any): any {
    console.log("executing");
    return {
        tracks: [],
    };
}

export default get_midi_track_data;
