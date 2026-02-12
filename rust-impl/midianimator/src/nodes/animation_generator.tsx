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
    const [name, setName] = useState(data.inputs?.name || ""); 

    useEffect(() => {
        getNodeData("animation_generator").then(setNodeData);
    }, []);

    useEffect(() => {
        setName(data.inputs?.name || "");
    }, [data.inputs?.name]);

    const handleUpdate = useCallback(() => {
        updateNodeData(id, { 
            ...data, 
            inputs: { 
                ...(data.inputs || {}), 
                name: name 
            } 
        });
    }, [id, data, name, updateNodeData]);

    const nameComponent = (
        <>
            <div>
                <input type="text" className="node-field border border-gray-400 rounded px-2 py-1" placeholder="Name" value={name} onChange={(e) => setName(e.target.value)} onBlur={handleUpdate} />
            </div>
        </>
    );


    const uiInject = {
        name: nameComponent,
    };

    const hiddenHandles = {
    };

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default animation_generator;
