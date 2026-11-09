import { useCallback, useEffect, useState } from "react";
import { message, open } from "@tauri-apps/plugin-dialog";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { useNodeSpec } from "../utils/nodeEntries";
import { useSetInputs } from "../utils/graphOps";
import { invoke } from "@tauri-apps/api/core";

// no stats, none show until the node has run
const NO_STATS = { tracks: 0, minutes: "0:00" };

function get_midi_file({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const nodeData = useNodeSpec("get_midi_file");
    // the file the node has, however it was set (here, Properties, MCP, undo)
    const filePath = data.inputs?.file_path;
    const fileName = typeof filePath === "string" ? filePath.split("/").pop() : "";

    const [fileStatsState, setFileStatsState] = useState<any>(NO_STATS);

    useEffect(() => {
        var fileStats: any = {};
        if (state != undefined && state.executed_results != undefined && id != undefined && id in state.executed_results) {
            // a failed node has no stats, only a motionkeys_error
            for (let line of (state.executed_results[id]["stats"] ?? "").split("\n")) {
                let res = line.split(" ");
                if (res[1] == "tracks") {
                    fileStats["tracks"] = res[0];
                } else if (res[1] == "minutes" || res[1] == "seconds") {
                    fileStats["minutes"] = line;
                }
            }
            setFileStatsState(fileStats);
            // state.executed_results[id]["stats"].split("\n").foreach((line: any) => {
            //     invoke("log", { message: JSON.stringify(line) });
            // });
        } else {
            // not run, or the run stopped before it: no stats left over from another file
            setFileStatsState(NO_STATS);
        }
    }, [state.executed_results]);

    const pick = useCallback(async () => {
        let res = await onMIDIFilePick();
        if (res != null) {
            console.log("updating data{} object");
            setInputs(id, { file_path: res.toString() });
        }
    }, [id, setInputs]);

    const filePathComponent = (
        <>
            <button onClick={pick} className="node-field bg-transparent font-semibold py-2 px-4 border border-black rounded">
                Pick MIDI File
            </button>
            <div className="node-field">{fileName}</div>

            {Object.keys(fileStatsState).length != 0 && fileStatsState.tracks != 0 ? (
                <>
                    <div className="node-field">
                        {fileStatsState.tracks} track{fileStatsState.tracks == 1 ? "" : "s"}
                    </div>
                    <div className="node-field">{fileStatsState.minutes}</div>
                </>
            ) : (
                <></>
            )}
        </>
    );

    const uiInject = {
        file_path: filePathComponent,
    };

    const hiddenHandles = {
        file_path: true,
        stats: true,
    };

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
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

export default get_midi_file;
