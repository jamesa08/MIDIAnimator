import { useEffect, useState } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { getNodeData } from "../utils/node";
import { useReactFlow } from "@xyflow/react";

function assign_notes_to_objects({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [nodeData, setNodeData] = useState<any | null>(null);

    useEffect(() => {
        getNodeData("assign_notes_to_objects").then(setNodeData);
    }, []);

    const uiInject = {
        
    };

    const hiddenHandles = {
        
    };

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default assign_notes_to_objects;
