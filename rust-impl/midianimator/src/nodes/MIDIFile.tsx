import { useCallback, useState } from "react";
import { open } from "@tauri-apps/api/dialog";
import "reactflow/dist/base.css";
import NodeHeader from "./NodeHeader";
import BaseNode from "./BaseNode";

function MIDIFileNode({ data, isConnectable }: { data: any; isConnectable: any }) {
    const pick = useCallback(async () => {
        let res = await onMIDIFilePick();
        if (res != null) {
            setFile(res.toString());
        }
    }, []);

    const onChange = useCallback((evt: any) => {
        console.log(evt.target.value);
    }, []);

    const [file, setFile] = useState("");
    const fileName = file.split("/").pop();

    const fileStats = {
        tracks: 0,
        minutes: "0:00",
    };

    const fields = {
        output: [
            {
                id: "tracks",
                name: "Tracks",
                type: "object",
                shape: "CIRCLE",
                component: <></>,
                show: true,
            },
        ],
        input: [
            {
                id: "file_hidden",
                name: "file",
                type: "string",
                shape: "CIRCLE",
                component: (
                    <>
                        <button onClick={pick} className="node-field bg-transparent font-semibold py-2 px-4 border border-black rounded">
                            Pick MIDI File
                        </button>
                        <div className="node-field">
                            {fileName}
                        </div>
                    </>
                ),
                show: false,
            },
        ],
    };
    

    return (
        <BaseNode handles={fields} data={data}>
            <NodeHeader label={"Get MIDI File"} type="TRANSFORM" />
        </BaseNode>
    );
}

async function onMIDIFilePick() {
    const selected = await open({
        multiple: false,
        filters: [
            {
                name: "",
                extensions: ["mid", "midi"],
            },
        ],
    });
    return selected;
}
export default MIDIFileNode;
