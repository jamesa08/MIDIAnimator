import { useCallback, useEffect, useState } from "react";
import { message, open } from "@tauri-apps/plugin-dialog";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { getNodeData } from "../utils/node";
import { useReactFlow } from "@xyflow/react";
import { invoke } from "@tauri-apps/api/core";

function animation_generator({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [nodeData, setNodeData] = useState<any | null>(null);
    const [name, setName] = useState(data.inputs?.name || ""); 
    const [property, setProperty] = useState(data.inputs?.animation_property || "");

    useEffect(() => {
        getNodeData("animation_generator").then(setNodeData);
    }, []);

    useEffect(() => {
        setName(data.inputs?.name || "");
    }, [data.inputs?.name]);

    useEffect(() => {
        setProperty(data.inputs?.animation_property || "");
    }, [data.inputs?.animation_property]);

    const handleUpdate = useCallback(() => {
        updateNodeData(id, { 
            ...data, 
            inputs: { 
                ...(data.inputs || {}), 
                name: name 
            } 
        });
    }, [id, data, name, updateNodeData]);

    // empty inherits the property from the note on keyframes
    const handlePropertyUpdate = useCallback(() => {
        updateNodeData(id, { ...data, inputs: { ...(data.inputs || {}), animation_property: property } });
    }, [id, data, property, updateNodeData]);

    const nameComponent = (
        <>
            <div>
                <input type="text" className="node-field border border-gray-400 rounded px-2 py-1" placeholder="Name" value={name} onChange={(e) => setName(e.target.value)} onBlur={handleUpdate} />
            </div>
        </>
    );

    const propertyComponent = (
        <div>
            <input type="text" className="node-field border border-gray-400 rounded px-2 py-1" placeholder="Inherit (e.g. location[2])" value={property} onChange={(e) => setProperty(e.target.value)} onBlur={handlePropertyUpdate} />
        </div>
    );

    const uiInject = {
        name: nameComponent,
        animation_property: propertyComponent,
    };

    const hiddenHandles = {
    };

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default animation_generator;
