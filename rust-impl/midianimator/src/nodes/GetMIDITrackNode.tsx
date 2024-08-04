import { useEffect, useState } from "react";
import { useReactFlow } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { getNodeData } from "../utils/node";
import { useStateContext } from "../contexts/StateContext";


function GetMIDITrackNode({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const [nodeData, setNodeData] = useState<any | null>(null);
    const { backEndState: state, setBackEndState: setState } = useStateContext();
    
    const [trackNamesState, setTrackNamesState] = useState<any>([""]);

    useEffect(() => {
        getNodeData("get_midi_track").then(setNodeData);
        updateNodeData(id, { ...data, inputs: { ...data.inputs, track_name: "Test" } });
    }, []);


    useEffect(() => {
        var trackNames = [];
        if (state != undefined && state.executed_inputs != undefined && id != undefined && id in state.executed_inputs) {
            for (let key in state.executed_inputs[id]["tracks"]) {
                let trackName = state.executed_inputs[id]["tracks"][key]["name"];
                trackNames.push(trackName);
            }

            if (trackNames.length == 0) {
                trackNames = ["No track names found"];
            }
            setTrackNamesState(trackNames);
        } else {
            setTrackNamesState(["No track names found"]);
        }
    }, [state.executed_inputs]);

    // this will need updated to reflect the the real data,
    // and how do we handle evaluating the data on change?
    // how do we handle sending the data to the backend?
    const trackNameComponent = (
        <>
            <select
                className="node-field nodrag nopan"
                onChange={(event) => {
                    updateNodeData(id, { ...data, inputs: { ...data.inputs, track_name: event.target.value } });
                }}
            >
                {trackNamesState.map((track: any) => {
                    return <option value={track}>{track}</option>;
                })}
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

export default GetMIDITrackNode;
