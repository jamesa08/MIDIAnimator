import { invoke } from "@tauri-apps/api/tauri";
import { useEffect, useState, useCallback, useRef } from "react";

import ReactFlow, { MiniMap, Controls, Background, useNodesState, useEdgesState, addEdge, Connection, Edge, BackgroundVariant, Position, ReactFlowInstance } from "reactflow";
import "reactflow/dist/style.css";
import nodeTypes from "../nodes/NodeTypes";
import { useStateContext } from "../contexts/StateContext";

const initialNodes = [
    { id: "1", position: { x: 0, y: 0 }, data: {}, type: "midi_file_node" },
    { id: "2", position: { x: 300, y: 0 }, data: {}, type: "midi_track_node" },
];
const initialEdges: any = [
    /*{ id: "e1-2", source: "1", target: "2" } */
];

function NodeGraph() {
    const [nodes, setNodes, onNodesChange] = useNodesState(undefined as any);
    const [edges, setEdges, onEdgesChange] = useEdgesState(undefined as any);
    const [rfInstance, setRfInstance] = useState(null as ReactFlowInstance | null);
    const { backEndState: state, setBackEndState: setState } = useStateContext();
    const initDone = useRef(false);

    const onConnect = useCallback(
        (params: Edge | Connection) => {
            console.log("onConnect", params);
            setEdges((eds) => addEdge(params, eds));

            // send update to backend

            let newState = { ...state, rf_instance: rfInstance?.toObject()};
            setState(newState);
            invoke("js_update_state", {"state": JSON.stringify(newState)});
        },
        [setEdges, state]
    );


    // on initalization & with the state readied, set the nodes and edges & update state to backend
    useEffect(() => {
        if (initDone.current == false && state.ready == true && state.rf_instance != undefined) {
            // set the nodes and edges
            // FIXME this will go away
            setNodes(initialNodes);
            setEdges(initialEdges);


            let newState = { ...state, rf_instance: rfInstance?.toObject()};
            setState(newState);
            invoke("js_update_state", {"state": JSON.stringify(newState)});
            initDone.current = true;  // only run once
        }
    }, [state]);

    
    const onInit = useCallback((instance: ReactFlowInstance) => {
        setRfInstance(instance);
    }, []);
    
    return (
        <ReactFlow nodes={nodes} edges={edges} onNodesChange={onNodesChange} onEdgesChange={onEdgesChange} onConnect={onConnect} nodeTypes={nodeTypes} onInit={onInit} fitView>
            <Background variant={BackgroundVariant.Dots} gap={12} size={1} />
            <Controls />
        </ReactFlow>
    );
}

export default NodeGraph;
