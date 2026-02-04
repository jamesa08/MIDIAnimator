import { invoke } from "@tauri-apps/api/tauri";
import { useEffect, useState, useCallback, useRef } from "react";

import { ReactFlow, MiniMap, Controls, Background, useNodesState, useEdgesState, addEdge, Connection, Edge, BackgroundVariant, Position, ReactFlowInstance, applyNodeChanges, applyEdgeChanges, useReactFlow, getOutgoers, ReactFlowProvider, useOnViewportChange, SelectionMode, getNodesBounds, useStoreApi } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import nodeTypes from "../nodes/NodeTypes";
import { useStateContext } from "../contexts/StateContext";
import ConnectionLine from "./ConnectionLine";

const initialNodes = [
    { id: "get_midi_file-8fb82482-a4bc-4b02-b238-64462daa3b56", position: { x: 0, y: 0 }, data: {}, type: "get_midi_file" },
    { id: "get_midi_track_data-5747a465-beac-45ab-b7ec-72d9e9d35947", position: { x: 300, y: 0 }, data: {}, type: "get_midi_track_data" },
    { id: "viewer-65630ce3-b9d1-4491-9936-d4e4c1d501d3", position: { x: 600, y: 0 }, data: {}, type: "viewer" },
    { id: "scene_link-0bfb3c01-6672-4665-a47e-6a6138b3c9ea", position: { x: 0, y: 300 }, data: {}, type: "scene_link" },
    { id: "keyframes_from_object-0bfb3c01-6672-4665-a47e-6a6138b3c9ea", position: { x: 300, y: 300 }, data: {}, type: "keyframes_from_object" },
    { id: "animation_generator-0bfb3c01-6672-4665-a47e-6a6138b3c9ea", position: { x: 600, y: 300 }, data: {}, type: "animation_generator" },
    { id: "assign_notes_to_objects-0bfb3c01-6672-4665-a47e-6a6138b3c9ea", position: { x: 900, y: 300 }, data: {}, type: "assign_notes_to_objects" },
];
const initialEdges: any = [
    /*{ id: "e1-2", source: "1", target: "2" } */
];

// ADD NODE MENU COMPONENT
function NodeAddMenu({ isOpen, onClose, onSelect, position }: { isOpen: boolean; onClose: () => void; onSelect: (nodeType: string) => void; position: { x: number; y: number } }) {
    const [search, setSearch] = useState("");
    const searchInputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        if (!isOpen) {
            setSearch("");
        }
    }, [isOpen]);

    const availableNodeTypes = Object.keys(nodeTypes);
    const filteredNodes = availableNodeTypes.filter((nodeType) => nodeType.toLowerCase().includes(search.toLowerCase()));

    useEffect(() => {
        if (isOpen && searchInputRef.current) {
            searchInputRef.current.focus();
        }
    }, [isOpen]);

    if (!isOpen) return null;

    return (
        <div
            style={{
                position: "fixed",
                left: position.x,
                top: position.y,
                backgroundColor: "#2a2a2a",
                border: "1px solid #444",
                borderRadius: "4px",
                width: "250px",
                maxHeight: "400px",
                zIndex: 1000,
                display: "flex",
                flexDirection: "column",
            }}
            onMouseDown={(e) => e.stopPropagation()}
        >
            <input
                ref={searchInputRef}
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search nodes..."
                style={{
                    padding: "8px",
                    backgroundColor: "#1a1a1a",
                    border: "none",
                    borderBottom: "1px solid #444",
                    color: "#fff",
                    outline: "none",
                }}
                onKeyDown={(e) => {
                    if (e.key === "Escape") {
                        onClose();
                    } else if (e.key === "Enter" && filteredNodes.length > 0) {
                        onSelect(filteredNodes[0]);
                    }
                }}
            />
            <div style={{ overflowY: "auto", maxHeight: "350px" }}>
                {filteredNodes.map((nodeType, index) => (
                    <div
                        key={nodeType}
                        onClick={() => onSelect(nodeType)}
                        style={{
                            padding: "8px 12px",
                            cursor: "pointer",
                            backgroundColor: "transparent",
                            color: "#fff",
                            fontSize: "14px",
                        }}
                        onMouseEnter={(e) => {
                            e.currentTarget.style.backgroundColor = "#4a7ba7";
                        }}
                        onMouseLeave={(e) => {
                            e.currentTarget.style.backgroundColor = "transparent";
                        }}
                    >
                        {nodeType.replace(/_/g, " ").replace(/\b\w/g, (l) => l.toUpperCase())}
                    </div>
                ))}
            </div>
        </div>
    );
}

function NodeGraphNoProvider() {
    const [nodes, setNodes] = useNodesState(undefined as any);
    const [edges, setEdges] = useEdgesState(undefined as any);

    const { getNodes, getEdges, screenToFlowPosition } = useReactFlow();
    const store = useStoreApi();

    const [rfInstance, setRfInstance] = useState(null as ReactFlowInstance | null);
    const [updateTrigger, setUpdateTrigger] = useState(false);

    const [menuOpen, setMenuOpen] = useState(false);
    const [menuPosition, setMenuPosition] = useState({ x: 0, y: 0 });
    const mousePositionRef = useRef({ x: 0, y: 0 });
    const [newNodeToDrag, setNewNodeToDrag] = useState<string | null>(null);
    const dragOffsetRef = useRef({ x: 0, y: 0 });

    //emulate 3 button mouse panning
    const [isPanningWithAlt, setIsPanningWithAlt] = useState(false);

    const { backEndState: state, setBackEndState: setState } = useStateContext();
    const initDone = useRef(false);

    const dragStartRef = useRef<{ cursorX: number; cursorY: number; nodes: Array<{ id: string; x: number; y: number }> }>({
        cursorX: 0,
        cursorY: 0,
        nodes: [],
    });

    const startDragging = useCallback(
        (nodeId: string, position?: { x: number; y: number }, offset = { x: 0, y: 0 }) => {
            if (position) {
                setNodes((nds) => nds.map((node) => (node.id === nodeId ? { ...node, position } : node)));
            }
            dragOffsetRef.current = offset;
            setNewNodeToDrag(nodeId);
        },
        [setNodes]
    );

    const stopDragging = useCallback(() => {
        // Make sure to keep nodes re-selected after drag
        if (newNodeToDrag && dragStartRef.current.nodes.length > 0) {
            const draggedNodeIds = new Set(dragStartRef.current.nodes.map((n) => n.id));

            setNodes((nds) =>
                nds.map((node) => ({
                    ...node,
                    selected: draggedNodeIds.has(node.id) ? true : node.selected,
                }))
            );
        }

        setNewNodeToDrag(null);
        setUpdateTrigger(true);
    }, []);

    // ADD NODE MENU HANDLERS
    // Add node creation function
    const addNode = useCallback(
        (nodeType: string) => {
            const newNodeId = `${nodeType}-${crypto.randomUUID()}`;
            const flowPosition = screenToFlowPosition(mousePositionRef.current, { snapToGrid: false });

            const newNode = {
                id: newNodeId,
                position: { x: flowPosition.x + 10, y: flowPosition.y + 10 },
                data: {},
                type: nodeType,
            };
            setNodes((nds) => [...nds, newNode]);

            // Store drag start for the new node
            dragStartRef.current = {
                cursorX: flowPosition.x - 10,
                cursorY: flowPosition.y - 10,
                nodes: [{ id: newNodeId, x: flowPosition.x, y: flowPosition.y }],
            };

            setNewNodeToDrag(newNodeId);
            setMenuOpen(false);
        },
        [setNodes, screenToFlowPosition, startDragging]
    );
    // Track mouse position
    useEffect(() => {
        const handleMouseMove = (event: MouseEvent) => {
            mousePositionRef.current = { x: event.clientX, y: event.clientY };
            // Add this line:
            if (menuOpen) {
                const flowPosition = screenToFlowPosition({ x: event.clientX, y: event.clientY }, { snapToGrid: false });
            }
        };

        window.addEventListener("mousemove", handleMouseMove);
        return () => window.removeEventListener("mousemove", handleMouseMove);
    }, [menuOpen, screenToFlowPosition]);

    useEffect(() => {
        if (!newNodeToDrag) return;

        const handleMouseMove = (event: MouseEvent) => {
            const flowPosition = screenToFlowPosition({ x: event.clientX, y: event.clientY }, { snapToGrid: false });

            // Calculate delta from start position
            const dx = flowPosition.x - dragStartRef.current.cursorX;
            const dy = flowPosition.y - dragStartRef.current.cursorY;

            setNodes((nds) => {
                if (!nds) return nds;
                return nds.map((node) => {
                    const draggedNode = dragStartRef.current.nodes.find((n) => n.id === node.id);
                    if (draggedNode) {
                        return {
                            ...node,
                            position: {
                                x: draggedNode.x + dx,
                                y: draggedNode.y + dy,
                            },
                        };
                    }
                    return node;
                });
            });
        };

        const handleMouseDown = (event: MouseEvent) => {
            stopDragging();
        };

        window.addEventListener("mousemove", handleMouseMove);
        window.addEventListener("mousedown", handleMouseDown);

        return () => {
            window.removeEventListener("mousemove", handleMouseMove);
            window.removeEventListener("mousedown", handleMouseDown);
        };
    }, [newNodeToDrag, screenToFlowPosition, setNodes]);

    // Stop dragging when clicking inside selected nodes. Need a better way to do this in the future.
    useEffect(() => {
        const handleMouseDown = (event: MouseEvent) => {
            if (!newNodeToDrag) return;

            // Get all selected nodes
            const selectedNodes = nodes.filter((node) => node.selected);
            if (selectedNodes.length === 0) return;

            // Convert mouse position to flow coordinates
            const flowPosition = screenToFlowPosition({ x: event.clientX, y: event.clientY });
            const { nodeLookup } = store.getState();

            const bounds = getNodesBounds(selectedNodes, { nodeLookup });

            // Check if click is inside the selection bounds
            const clickedInsideSelection = flowPosition.x >= bounds.x && flowPosition.x <= bounds.x + bounds.width && flowPosition.y >= bounds.y && flowPosition.y <= bounds.y + bounds.height;

            if (clickedInsideSelection) {
                stopDragging();
            }
        };

        window.addEventListener("mousedown", handleMouseDown, true);
        return () => window.removeEventListener("mousedown", handleMouseDown, true);
    }, [newNodeToDrag, nodes, screenToFlowPosition, stopDragging]);

    // Keyboard listener
    useEffect(() => {
        const handleKeyDown = (event: KeyboardEvent) => {
            // ignore if focused on input and not keybind
            const target = event.target as HTMLElement;
            if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable) {
                return;
            }

            if (event.shiftKey && event.key === "A") {
                event.preventDefault();
                const { x, y } = mousePositionRef.current;
                setMenuPosition({ x, y });
                setMenuOpen(true);
            } else if (event.key === "Escape") {
                setMenuOpen(false);
            } else if (event.key === "x") {
                event.preventDefault();
                // Delete selected nodes and edges
                setNodes((nds) => nds.filter((node) => !node.selected));
                setEdges((eds) => eds.filter((edge) => !edge.selected));
                setUpdateTrigger(true);
            } else if (event.key === "g") {
                event.preventDefault();
                const selectedNodes = nodes.filter((node) => node.selected);
                if (selectedNodes.length > 0) {
                    const { x, y } = mousePositionRef.current;
                    const flowPosition = screenToFlowPosition({ x, y }, { snapToGrid: false });

                    // Store initial cursor position and ALL selected node positions
                    dragStartRef.current = {
                        cursorX: flowPosition.x,
                        cursorY: flowPosition.y,
                        nodes: selectedNodes.map((node) => ({
                            id: node.id,
                            x: node.position.x,
                            y: node.position.y,
                        })),
                    };

                    setNewNodeToDrag("__multi_drag__"); // Use a special ID for multi-drag
                }
            } else if (event.shiftKey && event.key === "D") {
                event.preventDefault();
                const selectedNodes = nodes.filter((node) => node.selected);
                if (selectedNodes.length === 0) return;

                const { x, y } = mousePositionRef.current;
                const flowPosition = screenToFlowPosition({ x, y }, { snapToGrid: false });

                // Create a map of old node IDs to new node IDs
                const oldToNewIdMap = new Map<string, string>();

                const newNodes = selectedNodes.map((node) => {
                    const newNodeId = `${node.type}-${crypto.randomUUID()}`;
                    oldToNewIdMap.set(node.id, newNodeId);

                    return {
                        ...node,
                        id: newNodeId,
                        position: {
                            x: node.position.x + 20,
                            y: node.position.y + 20,
                        },
                        selected: true,
                        data: { ...node.data },
                    };
                });

                // Duplicate edges that connect duplicated nodes
                const selectedNodeIds = new Set(selectedNodes.map((n) => n.id));
                const newEdges = edges
                    .filter((edge) => selectedNodeIds.has(edge.source) && selectedNodeIds.has(edge.target))
                    .map((edge) => ({
                        ...edge,
                        id: `${crypto.randomUUID()}`,
                        source: oldToNewIdMap.get(edge.source)!,
                        target: oldToNewIdMap.get(edge.target)!,
                    }));

                // Deselect originals, add duplicates
                setNodes((nds) => [...nds.map((n) => ({ ...n, selected: false })), ...newNodes]);

                // Add duplicated edges
                setEdges((eds) => [...eds, ...newEdges]);

                // Store drag start positions for all duplicated nodes
                dragStartRef.current = {
                    cursorX: flowPosition.x,
                    cursorY: flowPosition.y,
                    nodes: newNodes.map((node) => ({
                        id: node.id,
                        x: node.position.x,
                        y: node.position.y,
                    })),
                };

                setNewNodeToDrag("__multi_drag__");
                setUpdateTrigger(true);
            }
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [screenToFlowPosition, setNodes, setEdges, nodes]);

    // Close menu on click outside
    useEffect(() => {
        const handleClick = () => {
            if (menuOpen) {
                setMenuOpen(false);
            }
        };
        window.addEventListener("mousedown", handleClick);

        return () => window.removeEventListener("mousedown", handleClick);
    }, [menuOpen]);

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

    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === "Alt") setIsPanningWithAlt(true);
        };
        const handleKeyUp = (e: KeyboardEvent) => {
            if (e.key === "Alt") setIsPanningWithAlt(false);
        };

        window.addEventListener("keydown", handleKeyDown);
        window.addEventListener("keyup", handleKeyUp);
        return () => {
            window.removeEventListener("keydown", handleKeyDown);
            window.removeEventListener("keyup", handleKeyUp);
        };
    }, []);

    // HANDLERS FOR REACT FLOW EVENTS
    const handlePaneClick = useCallback(
        (event: React.MouseEvent) => {
            if (newNodeToDrag) {
                stopDragging();
            }
        },
        [newNodeToDrag, stopDragging]
    );

    const handleNodeClickStop = useCallback(
        (event: React.MouseEvent, node: any) => {
            if (newNodeToDrag) {
                stopDragging();
            }
        },
        [newNodeToDrag, stopDragging]
    );

    const handleNodeDrag = useCallback(
        (event: React.MouseEvent, node: any) => {
            if (newNodeToDrag) {
                stopDragging();
            }
        },
        [newNodeToDrag, stopDragging]
    );

    useOnViewportChange({
        onStart: () => {
            if (newNodeToDrag) {
                stopDragging();
            }
        },
    });

    const onConnect = useCallback(
        (params: Edge | Connection) => {
            console.log("onConnect", params);

            // check if the connection is already present (target has an incoming edge)
            const existingEdgeIndex = edges.findIndex((edge) => edge.source == params.source && edge.sourceHandle == params.sourceHandle);

            if (existingEdgeIndex !== -1) {
                // if an edge exists to the target handle, replace it with the new connection
                setEdges((eds) => {
                    const updatedEdges = [...eds];
                    updatedEdges[existingEdgeIndex] = { ...params, id: updatedEdges[existingEdgeIndex].id };
                    return updatedEdges;
                });
            } else {
                // if no existing edge, simply add the new connection
                setEdges((eds) => addEdge(params, eds));
            }

            setUpdateTrigger(true);
        },
        [edges, setEdges, state, setUpdateTrigger]
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

            // prevent connecting to source node
            if (target?.id === connection.source) return false;
            return !hasCycle(target);
        },
        [getNodes, getEdges]
    );

    return (
        <>
            <ReactFlow nodes={nodes} edges={edges} onNodesChange={onNodesChange} onEdgesChange={onEdgesChange} onConnect={onConnect} onPaneClick={handlePaneClick} onNodeClick={handleNodeClickStop} onNodeDrag={handleNodeDrag} nodeTypes={nodeTypes} onInit={onInit} connectionLineComponent={ConnectionLine} isValidConnection={isValidConnection} panOnDrag={isPanningWithAlt ? true : [1]} selectionOnDrag={true} multiSelectionKeyCode={"Shift"} selectionKeyCode={"b"} selectionMode={SelectionMode.Partial} fitView>
                <Background variant={BackgroundVariant.Dots} gap={12} size={1} />
                <Controls />
                <MiniMap position="top-right" style={{ width: 100, height: 75 }} />
            </ReactFlow>
            <NodeAddMenu isOpen={menuOpen} onClose={() => setMenuOpen(false)} onSelect={addNode} position={menuPosition} />
        </>
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
