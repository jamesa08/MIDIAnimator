import { useEffect, useState } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { getNodeData } from "../utils/node";
import { useEdges, useUpdateNodeInternals } from "@xyflow/react";

function merge_object_maps({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const [nodeData, setNodeData] = useState<any | null>(null);
    const edges = useEdges();
    const updateNodeInternals = useUpdateNodeInternals();

    useEffect(() => {
        getNodeData("merge_object_maps").then(setNodeData);
    }, []);

    // one input per connected map plus a free one, same as node_inputs in model.rs
    // edges are stored inverted, source is this node's input
    const connected: number[] = edges
        .filter((e) => e.source === id && e.sourceHandle?.startsWith("object_maps_"))
        .map((e) => parseInt(e.sourceHandle!.slice("object_maps_".length)))
        .filter((i) => !isNaN(i));
    const indices = [...new Set(connected)].sort((a, b) => a - b);
    indices.push(indices.length > 0 ? indices[indices.length - 1] + 1 : 0);

    // handles changed, have React Flow measure them again so edges to the new one can be drawn
    const indicesKey = indices.join(",");
    useEffect(() => {
        updateNodeInternals(id);
    }, [id, indicesKey, updateNodeInternals]);

    const dynamicHandles = {
        inputs: indices.map((i) => ({ id: `object_maps_${i}`, name: "Object Map", data_type: "ObjectMap" })),
    };

    const uiInject = {};

    const hiddenHandles = {
        object_maps: true,
    };

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} dynamicHandles={dynamicHandles} data={data} />;
}

export default merge_object_maps;
