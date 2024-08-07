import { invoke } from "@tauri-apps/api/tauri";
import { useEffect, useState, useCallback, useRef } from "react";

import { ReactFlow, MiniMap, Controls, Background, useNodesState, useEdgesState, addEdge, Connection, Edge, BackgroundVariant, Position, ReactFlowInstance, applyNodeChanges, applyEdgeChanges, useReactFlow, getOutgoers, ReactFlowProvider } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import nodeTypes from "../nodes/NodeTypes";
import { useStateContext } from "../contexts/StateContext";
import ConnectionLine from "./ConnectionLine";

const initialNodes = [
    { id: "get_midi_file", position: { x: 0, y: 0 }, data: {}, type: "get_midi_file" },
    { id: "get_midi_track", position: { x: 300, y: 0 }, data: {}, type: "get_midi_track" },
    { id: "viewer", position: { x: 600, y: 0 }, data: {}, type: "viewer" },
];
const initialEdges: any = [
    /*{ id: "e1-2", source: "1", target: "2" } */
];

function NodeGraphNoProvider() {
    const [nodes, setNodes] = useNodesState(undefined as any);
    const [edges, setEdges] = useEdgesState(undefined as any);

    const { getNodes, getEdges } = useReactFlow();

    const [rfInstance, setRfInstance] = useState(null as ReactFlowInstance | null);
    const [updateTrigger, setUpdateTrigger] = useState(false);

    const { backEndState: state, setBackEndState: setState } = useStateContext();
    const initDone = useRef(false);

    useEffect(() => {
        if (updateTrigger && rfInstance) {
            // send update to backend
            let newState = { ...state, rf_instance: rfInstance?.toObject() };
            setState(newState);
            invoke("js_update_state", { state: JSON.stringify(newState) });
            setUpdateTrigger(false);

            // execute real-time nodes
            invoke("execute_graph", { realtime: true });
        }
    }, [rfInstance, updateTrigger]);

    const onConnect = useCallback(
        (params: Edge | Connection) => {
            console.log("onConnect", params);
            setEdges((eds) => addEdge(params, eds));

            invoke("log", { message: "EDGE ADDED" });
            setUpdateTrigger(true);
        },
        [setEdges, state]
    );

    const onNodesChange = useCallback(
        (changes: any) => {
            setNodes((nds) => applyNodeChanges(changes, nds));
            if (initDone.current && !state.ready && state.rf_instance == undefined && rfInstance == undefined) {
                return;
            }
            for (let change of changes) {
                if (change["type"] == "replace") { 
                    // update backend, node got replaced
                    setUpdateTrigger(true);
                }
            }
        },
        [setNodes, state]
    );

    const onEdgesChange = useCallback(
        (changes: any) => {
            setEdges((eds) => applyEdgeChanges(changes, eds));
            if (initDone.current && !state.ready && state.rf_instance == undefined && rfInstance == undefined) {
                return;
            }
            console.log(changes);
            for (let change of changes) {
                if (change["type"] == "remove") {
                    // update backend, node got replaced
                    setUpdateTrigger(true);
                }
            }
        },
        [setEdges, state]
    );

    // on initalization & with the state readied, set the nodes and edges & update state to backend
    useEffect(() => {
        if (!initDone.current && state.ready && state.rf_instance != undefined && rfInstance != undefined) {
            // set the nodes and edges
            // FIXME this will go away
            setNodes(initialNodes);
            setEdges(initialEdges);

            let newState = { ...state, rf_instance: rfInstance?.toObject() };
            setState(newState);
            invoke("js_update_state", { state: JSON.stringify(newState) });
            initDone.current = true; // only run once
        }
    }, [state, rfInstance]);

    const onInit = useCallback((instance: ReactFlowInstance) => {
        setRfInstance(instance);
        setUpdateTrigger(true);
    }, []);


    // prevent cyclitic connections
    const isValidConnection = useCallback(
        (connection: { target: string; source: string }) => {
            // we are using getNodes and getEdges helpers here
            // to make sure we create isValidConnection function only once
            const nodes = getNodes();
            const edges = getEdges();

            const target = nodes.find((node) => node.id === connection.target);
            const hasCycle = (node: any, visited = new Set()) => {
                if (visited.has(node.id)) return false;

                visited.add(node.id);

                for (const outgoer of getOutgoers(node, nodes, edges)) {
                    if (outgoer.id === connection.source) return true;
                    if (hasCycle(outgoer, visited)) return true;
                }
            };

            if (target?.id === connection.source) return false;
            return !hasCycle(target);
        },
        [getNodes, getEdges]
    );

    return (
        <ReactFlow nodes={nodes} edges={edges} onNodesChange={onNodesChange} onEdgesChange={onEdgesChange} onConnect={onConnect} nodeTypes={nodeTypes} onInit={onInit} connectionLineComponent={ConnectionLine} isValidConnection={isValidConnection} fitView>
            <Background variant={BackgroundVariant.Dots} gap={12} size={1} />
            <Controls />
        </ReactFlow>
    );
}

function NodeGraph() {
    return (
        <ReactFlowProvider>
            <NodeGraphNoProvider />
        </ReactFlowProvider>
    );
}

export default NodeGraph;
