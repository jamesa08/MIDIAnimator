import { useCallback, useEffect, useState } from "react";
import { message, open } from "@tauri-apps/api/dialog";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { getNodeData } from "../utils/node";
import { useReactFlow } from "@xyflow/react";
import { invoke } from "@tauri-apps/api/tauri";

function animation_generator({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [nodeData, setNodeData] = useState<any | null>(null);
    // const [file, setFile] = useState("");
    // const fileName = file.split("/").pop();

    // const [fileStatsState, setFileStatsState] = useState<any>({ tracks: 0, minutes: "0:00" });

    // useEffect(() => {
    //     var fileStats: any = {};
    //     if (state != undefined && state.executed_results != undefined && id != undefined && id in state.executed_results) {
    //         for (let line of state.executed_results[id]["stats"].split("\n")) {
    //             let res = line.split(" ");
    //             if (res[1] == "tracks") {
    //                 fileStats["tracks"] = res[0];
    //             } else if (res[1] == "minutes" || res[1] == "seconds") {
    //                 fileStats["minutes"] = line;
    //             }
    //         }
    //         setFileStatsState(fileStats);
    //     }
    // }, [state.executed_results]);

    useEffect(() => {
        getNodeData("animation_generator").then(setNodeData);
    }, []);

    // const pick = useCallback(async () => {
    //     let res = await onMIDIFilePick();
    //     if (res != null) {
    //         console.log("updating data{} object");
    //         setFile(res.toString());
    //         updateNodeData(id, { ...data, inputs: { ...data.inputs, file_path: res.toString() } });
    //     }
    // }, []);

    // const filePathComponent = (
    //     <>
    //         <button onClick={pick} className="node-field bg-transparent font-semibold py-2 px-4 border border-black rounded">
    //             Pick MIDI File
    //         </button>
    //         <div className="node-field">{fileName}</div>

    //         {Object.keys(fileStatsState).length != 0 && fileStatsState.tracks != 0 ? (
    //             <>
    //                 <div className="node-field">
    //                     {fileStatsState.tracks} track{fileStatsState.tracks == 1 ? "" : "s"}
    //                 </div>
    //                 <div className="node-field">{fileStatsState.minutes}</div>
    //             </>
    //         ) : (
    //             <></>
    //         )}
    //     </>
    // );

    const uiInject = {
        // file_path: filePathComponent,
    };

    const hiddenHandles = {
        // file_path: true,
    };

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default animation_generator;
