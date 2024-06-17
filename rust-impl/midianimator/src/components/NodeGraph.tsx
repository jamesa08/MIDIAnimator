import { open } from "@tauri-apps/api/dialog";
import { invoke } from "@tauri-apps/api/tauri";
import React, { useEffect, useState, useCallback } from "react";

import ReactFlow, { MiniMap, Controls, Background, useNodesState, useEdgesState, addEdge, Connection, Edge, BackgroundVariant, Position } from "reactflow";
import "reactflow/dist/style.css";
import MIDIFileNode from "../nodes/MIDIFile";
import nodeTypes from "../NodeTypes";

const nodeDefaults = {
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
};

const initialNodes = [
    { id: "1", position: { x: 0, y: 0 }, data: { label: "Track" }, type: "midi_file_node" },
];
const initialEdges: any = [ /*{ id: "e1-2", source: "1", target: "2" } */];

function NodeGraph() {
    const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
    const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

    const onConnect = useCallback((params: Edge | Connection) => setEdges((eds) => addEdge(params, eds)), [setEdges]);

    return (
        <ReactFlow nodes={nodes} edges={edges} onNodesChange={onNodesChange} onEdgesChange={onEdgesChange} onConnect={onConnect} nodeTypes={nodeTypes} fitView>
            <Background variant={BackgroundVariant.Dots} gap={12} size={1} />
            <Controls />
        </ReactFlow>
    );
}

export default NodeGraph;
