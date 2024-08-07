import { useCallback, useEffect, useState } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { getNodeData } from "../utils/node";
import { useReactFlow } from "@xyflow/react";
import { stringify } from "flatted";

function ViewerNode({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [nodeData, setNodeData] = useState<any | null>(null);
    const [viewerData, setViewerData] = useState<any | null>(null);

    useEffect(() => {
        var tempViewerData: any = "";
        if (state != undefined && state.executed_results != undefined && id != undefined) {
            if (id in state.executed_inputs && id in state.executed_results) {
                console.log(state.executed_inputs);
                tempViewerData = stringify(state.executed_inputs[id]["data"]);
                // if (state.executed_inputs[id]["data"] == null || state.executed_inputs[id]["data"].length == 0) {
                //     console.log("no executed inputs");
                //     viewerData = "null";
                // } else {
                //     for (let line of state.executed_inputs[id]["data"]) {
                //         viewerData += stringify(line) + "\n";
                //     }
                // }
                if (tempViewerData != viewerData) {
                    setViewerData(tempViewerData);
                }
            } else {
                setViewerData("");
            }
        }
    }, [state.executed_results]);

    useEffect(() => {
        getNodeData("viewer").then(setNodeData);
    }, []);

    const viewerComponent = (
        <>
            <div className="node-field node-viewer">{viewerData}</div>
        </>
    );

    const uiInject = {
        data: viewerComponent,
    };

    const hiddenHandles = {};

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default ViewerNode;
