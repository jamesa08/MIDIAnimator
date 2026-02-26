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
    }, []);

    function arraysEqual(arr1: any[], arr2: string | any[]) {
        if (arr1.length !== arr2.length) return false;

        return arr1.every((item, index) => {
            return item === arr2[index];
        });
    }

    useEffect(() => {
        if (state?.executed_inputs == undefined || !(id in state.executed_inputs)) {
            // No execution data — preserve current values as the sole option
            const preservedGroup = data.inputs?.object_group_name || "";
            const preservedObject = data.inputs?.object_name || "";
            setObjectGroupNameState(preservedGroup ? [preservedGroup] : ["No ObjectGroup names found"]);
            setObjectNameState(preservedObject ? [preservedObject] : ["No Object names found"]);
            return;
        }

        // Object group names
        const objectGroupNames: string[] = [];
        for (let key in state.executed_inputs[id]["object_groups"]) {
            objectGroupNames.push(state.executed_inputs[id]["object_groups"][key]["name"]);
        }

        if (objectGroupNames.length === 0) {
            setObjectGroupNameState(data.inputs?.object_group_name ? [data.inputs.object_group_name] : ["No ObjectGroup names found"]);
        } else if (!arraysEqual(objectGroupNames, objectGroupNameState)) {
            setObjectGroupNameState(objectGroupNames);
        }

        // Object names
        const objectNames: string[] = [];
        if ("object_name" in state.executed_inputs[id]) {
            const objectGroupName = state.executed_inputs[id]["object_group_name"];
            for (let key in state.executed_inputs[id]["object_groups"]) {
                if (state.executed_inputs[id]["object_groups"][key]["name"] === objectGroupName) {
                    for (let objectKey in state.executed_inputs[id]["object_groups"][key]["objects"]) {
                        objectNames.push(state.executed_inputs[id]["object_groups"][key]["objects"][objectKey]["name"]);
                    }
                }
            }
        }

        if (objectNames.length === 0) {
            setObjectNameState(data.inputs?.object_name ? [data.inputs.object_name] : ["No Object names found"]);
        } else if (!arraysEqual(objectNames, objectNameState)) {
            setObjectNameState(objectNames);
            // Only update node data if the current value isn't in the new list
            if (!objectNames.includes(data.inputs?.object_name)) {
                updateNodeData(id, { ...data, inputs: { ...data.inputs, object_name: objectNames[0] } });
            }
        }
    }, [state.executed_inputs]);

    let xyz = ["x", "y", "z"];
    // update dynamic handles
    useEffect(() => {
        if (state?.executed_results == undefined || !(id in state.executed_results)) {
            // No execution data — preserve current handles as-is
            return;
        }

        const dynOutput = state.executed_results[id]["dyn_output"];
        if (!dynOutput || dynOutput.length === 0) {
            return;
        }

        const animCurves = [];
        for (let animCurve of dynOutput) {
            let animCurveName = animCurve["data_path"] + "_";
            if (["location", "rotation", "scale"].indexOf(animCurve["data_path"]) !== -1) {
                animCurveName += xyz[animCurve["array_index"]];
            } else {
                animCurveName += animCurve["array_index"];
            }
            animCurves.push({
                id: animCurveName,
                name: animCurveName.split("_").join(" ").toProperCase(),
                type: "Array<Keyframe>",
            });
        }

        // Deep compare by id to avoid clobbering identical handles
        const sameHandles = animCurves.length === animCurveState.length && animCurves.every((c, i) => c.id === animCurveState[i]?.id);

        if (!sameHandles) {
            setAnimCurveState(animCurves);
        }
    }, [state.executed_results]);

    const objectGroupNameComponent = (
        <select
            className="node-field nodrag nopan"
            value={data.inputs?.object_group_name || ""}
            onChange={(event) => {
                updateNodeData(id, {
                    ...data,
                    inputs: {
                        ...data.inputs,
                        object_group_name: event.target.value,
                    },
                });
            }}
        >
            {objectGroupNameState.map((track: any, index: any) => (
                <option key={index} value={track}>
                    {track}
                </option>
            ))}
        </select>
    );

    const objectNameComponent = (
        <>
            <select
                className="node-field nodrag nopan"
                value={data.inputs?.object_name || ""}
                onChange={(event) => {
                    updateNodeData(id, {
                        ...data,
                        inputs: {
                            ...data.inputs,
                            object_name: event.target.value,
                        },
                    });
                }}
            >
                {objectNameState.map((track: any, index: any) => {
                    return (
                        <option key={index} value={track}>
                            {track}
                        </option>
                    );
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
