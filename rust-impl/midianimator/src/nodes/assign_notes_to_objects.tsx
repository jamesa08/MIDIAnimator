import { useEffect, useState, useCallback } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { getNodeData } from "../utils/node";
import { useReactFlow } from "@xyflow/react";

function assign_notes_to_objects({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [nodeData, setNodeData] = useState<any | null>(null);
    const [name, setName] = useState(data.inputs?.object_group_name || "");

    useEffect(() => {
        getNodeData("assign_notes_to_objects").then(setNodeData);
    }, []);

    useEffect(() => {
        setName(data.inputs?.object_group_name || "");
    }, [data.inputs?.object_group_name]);

    const handleUpdate = useCallback(() => {
        updateNodeData(id, { 
            ...data, 
            inputs: { 
                ...(data.inputs || {}), 
                object_group_name: name 
            } 
        });
    }, [id, data, name, updateNodeData]);

    const objectGroupNameComponent = (
        <>
            <div>
                <input type="text" className="node-field border border-gray-400 rounded px-2 py-1" placeholder="Object Group Name" value={name} onChange={(e) => setName(e.target.value)} onBlur={handleUpdate} />
            </div>
        </>
    );

    const uiInject = {
        object_group_name: objectGroupNameComponent,
    };

    const hiddenHandles = {};

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default assign_notes_to_objects;
