import { useCallback, useState } from "react";
import { open } from "@tauri-apps/api/dialog";
import "reactflow/dist/base.css";
import NodeHeader from "./NodeHeader";
import BaseNode from "./BaseNode";

function MIDITrackNode({ data, isConnectable }: { data: any; isConnectable: any }) {
    const onChange = useCallback((evt: any) => {
        console.log(evt.target.value);
    }, []);

    const fields = {
        output: [
            {
                id: "track",
                name: "Track",
                type: "MIDITrack",
                shape: "CIRCLE",
                component: <></>,
                show: true,
            },
        ],
        input: [
            {
                id: "tracks",
                name: "Tracks",
                type: "List[MIDITracks]",
                shape: "CIRCLE",
                component: (
                    <>
                        <select className="node-field" id="cars" name="cars">
                            <option value="volvo">Volvo</option>
                            <option value="saab">Saab</option>
                            <option value="fiat">Fiat</option>
                            <option value="audi">Audi</option>
                        </select>
                    </>
                ),
                show: true,
            },
        ],
    };

    return (
        <BaseNode handles={fields} data={data}>
            <NodeHeader label={"Get MIDI Track"} type="TRANSFORM" />
        </BaseNode>
    );
}

export default MIDITrackNode;
