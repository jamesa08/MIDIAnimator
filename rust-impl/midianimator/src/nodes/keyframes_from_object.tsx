import { useReactFlow } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import { useEffect, useState } from "react";
import BaseNode from "./BaseNode";
import { getNodeData } from "../utils/node";
import { useStateContext } from "../contexts/StateContext";

function keyframes_from_object({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const [nodeData, setNodeData] = useState<any | null>(null);
    const { backEndState: state } = useStateContext();

    useEffect(() => {
        getNodeData("keyframes_from_object").then(setNodeData);
    }, []);

    // Derive everything from state and data directly
    const executedInputs = state?.executed_inputs?.[id];
    const executedResults = state?.executed_results?.[id];

    const objectGroupNames: string[] = executedInputs ? executedInputs["object_groups"].map((g: any) => g.name) : [];

    const selectedGroupName: string = data.inputs?.object_group_name || objectGroupNames[0] || "";

    const objectNames: string[] = objectGroupNames.length > 0 ? executedInputs["object_groups"].find((g: any) => g.name === selectedGroupName)?.objects.map((o: any) => o.name) ?? [] : [];

    const selectedObjectName: string = data.inputs?.object_name || objectNames[0] || "";

    const animCurves = executedResults?.dyn_output
        ? Object.keys(executedResults.dyn_output).map((curveName) => ({
              id: curveName,
              name: curveName.split("_").join(" ").toProperCase(),
              type: "Array<Keyframe>",
          }))
        : [];

    useEffect(() => {
        console.log("state.executed_results[id] changed:", state?.executed_results?.[id]);
        if (selectedGroupName && selectedGroupName !== data.inputs?.object_group_name) {
            updateNodeData(id, { ...data, inputs: { ...data.inputs, object_group_name: selectedGroupName, object_name: selectedObjectName } });
        }
    }, [selectedGroupName, selectedObjectName]);

    const objectGroupNameComponent = (
        <select className="node-field nodrag nopan" value={selectedGroupName} onChange={(e) => updateNodeData(id, { ...data, inputs: { ...data.inputs, object_group_name: e.target.value } })}>
            {objectGroupNames.length > 0 ? (
                objectGroupNames.map((name, i) => (
                    <option key={i} value={name}>
                        {name}
                    </option>
                ))
            ) : (
                <option value="">No ObjectGroup names found</option>
            )}
        </select>
    );

    const objectNameComponent = (
        <select className="node-field nodrag nopan" value={selectedObjectName} onChange={(e) => updateNodeData(id, { ...data, inputs: { ...data.inputs, object_name: e.target.value } })}>
            {objectNames.length > 0 ? (
                objectNames.map((name, i) => (
                    <option key={i} value={name}>
                        {name}
                    </option>
                ))
            ) : (
                <option value="">No Object names found</option>
            )}
        </select>
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

    const dynamicHandles: any = {
        outputs: animCurves,
    };

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} dynamicHandles={dynamicHandles} data={data} />;
}

export default keyframes_from_object;
