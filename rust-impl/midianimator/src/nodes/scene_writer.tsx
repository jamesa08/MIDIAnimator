import { useCallback, useEffect, useState } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { getNodeData } from "../utils/node";
import { useReactFlow } from "@xyflow/react";

function scene_writer({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [nodeData, setNodeData] = useState<any | null>(null);
    const [viewerData, setViewerData] = useState<any | null>(null);

    useEffect(() => {
        getNodeData("scene_writer").then(setNodeData);
    }, []);

    const uiInject = {};

    const hiddenHandles = {};

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default scene_writer;
