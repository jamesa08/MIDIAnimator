import { useCallback, useEffect, useState } from "react";
import { message, open } from "@tauri-apps/plugin-dialog";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { getNodeData } from "../utils/node";
import { useSetInputs } from "../utils/graphOps";
import { invoke } from "@tauri-apps/api/core";

// the animation overlap modes, same order and ids as ANIMATION_OVERLAPS in the backend
const overlapModes = [
    { id: "add", name: "Add" },
    { id: "min", name: "Min" },
    { id: "max", name: "Max" },
    { id: "prev", name: "Previous" },
    { id: "next", name: "Next" },
    { id: "rvc", name: "Rest Value Crossing" },
    { id: "prune", name: "Keyframe Pruning" },
    { id: "crossfade", name: "Crossfade" },
];

function animation_generator({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [nodeData, setNodeData] = useState<any | null>(null);
    const [name, setName] = useState(data.inputs?.name || ""); 
    const [property, setProperty] = useState(data.inputs?.animation_property || "");
    const [blend, setBlend] = useState(data.inputs?.overlap_blend ?? "");

    useEffect(() => {
        getNodeData("animation_generator").then(setNodeData);
    }, []);

    useEffect(() => {
        setName(data.inputs?.name || "");
    }, [data.inputs?.name]);

    useEffect(() => {
        setProperty(data.inputs?.animation_property || "");
    }, [data.inputs?.animation_property]);

    useEffect(() => {
        setBlend(data.inputs?.overlap_blend ?? "");
    }, [data.inputs?.overlap_blend]);

    const handleUpdate = useCallback(() => {
        setInputs(id, { name });
    }, [id, name, setInputs]);

    // empty inherits the property from the note on keyframes
    const handlePropertyUpdate = useCallback(() => {
        setInputs(id, { animation_property: property });
    }, [id, property, setInputs]);

    // empty unsets it so the backend default is used, anything that isn't a number is ignored
    const handleBlendUpdate = useCallback(() => {
        if (blend === "") {
            setInputs(id, { overlap_blend: null });
        } else if (!isNaN(Number(blend))) {
            setInputs(id, { overlap_blend: Number(blend) });
        } else {
            setBlend(data.inputs?.overlap_blend ?? "");
        }
    }, [id, data, blend, setInputs]);

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

    // unset is the default, add
    const overlapComponent = (
        <>
            <div className="node-field field-inputs">
                <span>Animation Overlap</span>
            </div>
            <select className="node-field nodrag nopan" value={data.inputs?.animation_overlap || "add"} onChange={(e) => setInputs(id, { animation_overlap: e.target.value })}>
                {overlapModes.map((mode) => (
                    <option key={mode.id} value={mode.id}>
                        {mode.name}
                    </option>
                ))}
            </select>
        </>
    );

    // only crossfade uses the blend time
    const blendComponent =
        data.inputs?.animation_overlap === "crossfade" ? (
            <>
                <div className="node-field field-inputs">
                    <span>Blend (s)</span>
                </div>
                <input type="number" min="0" step="0.05" className="node-field nodrag border border-gray-400 rounded px-2 py-1" placeholder="0.1" value={blend} onChange={(e) => setBlend(e.target.value)} onBlur={handleBlendUpdate} />
            </>
        ) : (
            <></>
        );

    const uiInject = {
        name: nameComponent,
        animation_overlap: overlapComponent,
        overlap_blend: blendComponent,
        animation_property: propertyComponent,
    };

    const hiddenHandles = {
        animation_overlap: true,
        overlap_blend: true,
    };

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default animation_generator;
