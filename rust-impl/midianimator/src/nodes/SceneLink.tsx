import { useCallback, useEffect, useState } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { getNodeData } from "../utils/node";
import { useReactFlow } from "@xyflow/react";
import { stringify } from "flatted";

function SceneLink({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [nodeData, setNodeData] = useState<any | null>(null);

    useEffect(() => {
        if (state != undefined && state.executed_results != undefined && id != undefined) {
            if (id in state.executed_inputs && id in state.executed_results) {
                // executed
            } else {
                // not executed   
            }
        }
    }, [state.executed_results]);

    useEffect(() => {
        getNodeData("scene_link").then(setNodeData);
    }, []);


    const uiInject = {};

    const hiddenHandles = {};

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default SceneLink;
