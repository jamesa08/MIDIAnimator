import { useEffect, useState } from "react";
import { useReactFlow } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { getNodeData } from "../utils/node";
import { useStateContext } from "../contexts/StateContext";
import { invoke } from "@tauri-apps/api/tauri";

function keyframes_from_object({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const [nodeData, setNodeData] = useState<any | null>(null);
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [objectGroupNameState, setObjectGroupNameState] = useState<any>([""]);
    const [objectNameState, setObjectNameState] = useState<any>([""]);
    const [animCurveState, setAnimCurveState] = useState<any>([""]);

    useEffect(() => {
        getNodeData("keyframes_from_object").then(setNodeData);
        updateNodeData(id, { ...data, inputs: { ...data.inputs, object_group_name: "", object_name: "" } });
    }, []);

    function arraysEqual(arr1: any[], arr2: string | any[]) {
        if (arr1.length !== arr2.length) return false;

        return arr1.every((item, index) => {
            return item === arr2[index];
        });
    }

    useEffect(() => {
        var objectGroupNames = [];
        if (state != undefined && state.executed_inputs != undefined && id != undefined && id in state.executed_inputs) {
            for (let key in state.executed_inputs[id]["object_groups"]) {
                let objectGroupName = state.executed_inputs[id]["object_groups"][key]["name"];
                objectGroupNames.push(objectGroupName);
            }

            if (objectGroupNames.length == 0) {
                objectGroupNames = ["No Object Group names found"];
            }

            // check if we need to update the object group name
            if (!arraysEqual(objectGroupNames, objectGroupNameState)) {
                setObjectGroupNameState(objectGroupNames);
            }
        } else {
            setObjectGroupNameState(["No ObjectGroup names found"]);
        }

        var objectNames = [];
        if (state != undefined && state.executed_inputs != undefined && id != undefined && id in state.executed_inputs) {
            // check if objectname exists in the executed_inputs
            invoke("log", {message: JSON.stringify(state.executed_inputs[id])});
            if ("object_name" in state.executed_inputs[id]) {
                let objectGroupName = state.executed_inputs[id]["object_group_name"];
                // now iterate over the objects in that object group
                for (let key in state.executed_inputs[id]["object_groups"]) {
                    if (state.executed_inputs[id]["object_groups"][key]["name"] == objectGroupName) {
                        for (let objectKey in state.executed_inputs[id]["object_groups"][key]["objects"]) {
                            let objectName = state.executed_inputs[id]["object_groups"][key]["objects"][objectKey]["name"];
                            objectNames.push(objectName);
                        }
                    }
                }
            }

            if (objectNames.length == 0) {
                objectNames = ["No Object names found"];
            }

            invoke("log", { message: JSON.stringify(objectNames) });
            // need a more elaborate check, need to just check
            if (!arraysEqual(objectNames, objectNameState)) {
                setObjectNameState(objectNames);
                updateNodeData(id, { ...data, inputs: { ...data.inputs, object_name: objectNames[0] } });
            }
        } else {
            setObjectNameState(["No Object names found"]);
        }
    }, [state.executed_inputs]);

    let xyz = ["x", "y", "z"];
    // update dynamic handles
    useEffect(() => {
        var animCurves = [];
        if (state != undefined && state.executed_results != undefined && id in state.executed_results && state.executed_results[id]["dyn_output"] != undefined) {
            let animCurvesExecuted = state.executed_results[id]["dyn_output"].length != 0 && state.executed_results[id]["dyn_output"].length != undefined ? state.executed_results[id]["dyn_output"] : [];
            for (let animCurve of animCurvesExecuted) {
                let animCurveName = animCurve["data_path"] + "_";
                // check if the data path is location, rotation, or scale
                if (["location", "rotation", "scale"].indexOf(animCurve["data_path"]) !== -1) {
                    // convert to x, y, z
                    animCurveName += xyz[animCurve["array_index"]];
                } else {
                    animCurveName += animCurve["array_index"];
                }
                /* prettier-ignore */
                let animCurveHandle = {
                    "id": animCurveName,
                    "name": animCurveName.split("_").join(" ").toProperCase(),
                    "type": "Array<Keyframe>", 
                }
                animCurves.push(animCurveHandle);
            }

            if (!arraysEqual(animCurves, animCurveState)) {
                setAnimCurveState(animCurves);
            }
        }
    }, [state.executed_results]);

    const objectGroupNameComponent = (
        <>
            <select
                className="node-field nodrag nopan"
                onChange={(event) => {
                    updateNodeData(id, { ...data, inputs: { ...data.inputs, object_group_name: event.target.value } });
                }}
            >
                {objectGroupNameState.map((track: any) => {
                    return (
                        <option value={track} selected={state.executed_inputs != undefined && state.executed_inputs[id] != undefined && state.executed_inputs[id]["object_group_name"] != undefined && state.executed_inputs[id]["object_group_name"] == track}>
                            {track}
                        </option>
                    );
                })}
            </select>
        </>
    );

    const objectNameComponent = (
        <>
            <select
                className="node-field nodrag nopan"
                onChange={(event) => {
                    updateNodeData(id, { ...data, inputs: { ...data.inputs, object_name: event.target.value } });
                }}
            >
                {objectNameState.map((track: any) => {
                    return <option value={track}>{track}</option>;
                })}
            </select>
        </>
    );

    const uiInject = {
        object_group_name: objectGroupNameComponent,
        object_name: objectNameComponent,
    };

    const hiddenHandles = {
        object_group_name: true,
        object_name: true,
        dyn_output: true,
    };

    /* prettier-ignore */
    const dynamicHandles: any = {
        "outputs": animCurveState
    };

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} dynamicHandles={dynamicHandles} data={data} />;
}

export default keyframes_from_object;
