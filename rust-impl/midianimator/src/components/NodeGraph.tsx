import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, useCallback, useRef } from "react";

import { ReactFlow, MiniMap, Controls, Background, useNodesState, useEdgesState, addEdge, Connection, Edge, BackgroundVariant, Position, ReactFlowInstance, applyNodeChanges, applyEdgeChanges, useReactFlow, getOutgoers, ReactFlowProvider, useOnViewportChange, SelectionMode, useStoreApi, useNodesInitialized } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import nodeTypes from "../nodes/NodeTypes";
import { useStateContext } from "../contexts/StateContext";
import ConnectionLine from "./ConnectionLine";
import { NODE_DROP_EVENT, PROJECT_LOADED_EVENT } from "../utils/node";

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

// short node ids: `{type}-{N}` where N is one more than the highest N already used for that type.
// older projects use `{type}-{uuid}` ids, those are ignored here and keep working.
// keep in sync with Graph::next_node_id in src-tauri/src/graph/model.rs
function nextNodeId(nodes: { id: string }[], nodeType: string): string {
    // find the highest N already used for this type
    const prefix = `${nodeType}-`;
    let max = 0;
    for (const node of nodes) {
        if (!node.id.startsWith(prefix)) continue;
        // only count ids where the rest is a plain number (skips uuid ids)
        const rest = node.id.slice(prefix.length);
        if (/^\d+$/.test(rest)) max = Math.max(max, parseInt(rest, 10));
    }
    return `${prefix}${max + 1}`;
}

// the parts of a graph the backend actually owns, used to tell if a backend push changed anything
function graphKey(graph: any): string {
    const nodes = (graph.nodes ?? []).map(({ selected, dragging, measured, ...node }: any) => node);
    const edges = (graph.edges ?? []).map(({ selected, ...edge }: any) => edge);
    return JSON.stringify({ nodes, edges });
}

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
            }}
            className="bg-[#2a2a2a] border border-[#444] rounded w-[250px] max-h-[400px] z-[1000] flex flex-col"
            onMouseDown={(e) => e.stopPropagation()}
        >
            <input
                ref={searchInputRef}
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search nodes..."
                className="px-2 py-1 bg-[#1a1a1a] border-0 border-b border-[#444] text-white outline-none text-[13px]"
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
                            backgroundColor: "transparent",
                        }}
                        className="px-2 py-1 cursor-pointer text-white text-[13px]"
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
    // like updateTrigger but only sends the graph, doesn't re-execute (moves)
    const [syncTrigger, setSyncTrigger] = useState(false);

    const [menuOpen, setMenuOpen] = useState(false);
    const [menuPosition, setMenuPosition] = useState({ x: 0, y: 0 });
    const mousePositionRef = useRef({ x: 0, y: 0 });
    const [newNodeToDrag, setNewNodeToDrag] = useState<string | null>(null);
    const dragOffsetRef = useRef({ x: 0, y: 0 });

    //emulate 3 button mouse panning
    const [isPanningWithAlt, setIsPanningWithAlt] = useState(false);

    // cancel event params
    const preOperationStateRef = useRef<{
        nodes: any[];
        edges: any[];
    } | null>(null);

    const { backEndState: state, setBackEndState: setState } = useStateContext();
    const initDone = useRef(false);

    // set by the click that places nodes, the rest of that click gets swallowed
    const swallowClickRef = useRef(false);

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
        // keep the placed nodes selected after the drag
        if (dragStartRef.current.nodes.length > 0) {
            const draggedNodeIds = new Set(dragStartRef.current.nodes.map((n) => n.id));

            setNodes((nds) =>
                nds.map((node) => ({
                    ...node,
                    selected: draggedNodeIds.has(node.id) ? true : node.selected,
                }))
            );
        }

        // the operation is confirmed, nothing to cancel back to anymore
        dragStartRef.current = { cursorX: 0, cursorY: 0, nodes: [] };
        preOperationStateRef.current = null;
        setNewNodeToDrag(null);
        // tell the backend where the nodes ended up
        setSyncTrigger(true);
    }, [setNodes]);

    // cancel handlers

    // Function to save current state before an operation
    const savePreOperationState = useCallback(() => {
        // read from the store so the snapshot is never stale
        preOperationStateRef.current = {
            nodes: JSON.parse(JSON.stringify(getNodes())),
            edges: JSON.parse(JSON.stringify(getEdges())),
        };
    }, [getNodes, getEdges]);

    // Function to cancel and restore
    const cancelOperation = useCallback(() => {
        if (preOperationStateRef.current) {
            setNodes(preOperationStateRef.current.nodes);
            setEdges(preOperationStateRef.current.edges);
            preOperationStateRef.current = null;
            // the operation may already have been sent to the backend (shift+d)
            setUpdateTrigger(true);
        }

        dragStartRef.current = { cursorX: 0, cursorY: 0, nodes: [] };
        setNewNodeToDrag(null);
        setMenuOpen(false);
    }, [setNodes, setEdges]);

    // close the add menu without adding anything
    const closeMenu = useCallback(() => {
        preOperationStateRef.current = null;
        setMenuOpen(false);
    }, []);

    // ADD NODE MENU HANDLERS
    // Add node creation function
    const addNode = useCallback(
        (nodeType: string) => {
            // get the next short id for this node type
            const newNodeId = nextNodeId(getNodes(), nodeType);
            const flowPosition = screenToFlowPosition(mousePositionRef.current, { snapToGrid: false });

            const newNode = {
                id: newNodeId,
                position: { x: flowPosition.x + 10, y: flowPosition.y + 10 },
                data: {},
                type: nodeType,
                selected: true,
            };
            // the new node becomes the only selection, like blender
            setNodes((nds) => [...nds.map((n) => (n.selected ? { ...n, selected: false } : n)), newNode]);
            setEdges((eds) => (eds ?? []).map((e) => (e.selected ? { ...e, selected: false } : e)));

            // Store drag start for the new node
            dragStartRef.current = {
                cursorX: flowPosition.x - 10,
                cursorY: flowPosition.y - 10,
                nodes: [{ id: newNodeId, x: flowPosition.x, y: flowPosition.y }],
            };

            setNewNodeToDrag(newNodeId);
            setMenuOpen(false);
        },
        [setNodes, setEdges, getNodes, screenToFlowPosition]
    );
    // node dragged in from the nodes panel, added where it was released
    useEffect(() => {
        const handleDrop = (event: Event) => {
            const { nodeType, clientX, clientY, offsetX, offsetY } = (event as CustomEvent).detail;

            // only when released over the graph
            const rect = store.getState().domNode?.getBoundingClientRect();
            if (!rect || clientX < rect.left || clientX > rect.right || clientY < rect.top || clientY > rect.bottom) return;

            // keep the node under the cursor where it was grabbed
            const flowPosition = screenToFlowPosition({ x: clientX, y: clientY }, { snapToGrid: false });
            const newNode = {
                id: nextNodeId(getNodes(), nodeType),
                position: { x: flowPosition.x - offsetX, y: flowPosition.y - offsetY },
                data: {},
                type: nodeType,
                selected: true,
            };

            // the new node becomes the only selection, like adding from the menu
            setNodes((nds) => [...(nds ?? []).map((n) => (n.selected ? { ...n, selected: false } : n)), newNode]);
            setEdges((eds) => (eds ?? []).map((e) => (e.selected ? { ...e, selected: false } : e)));
            setSyncTrigger(true);
        };

        window.addEventListener(NODE_DROP_EVENT, handleDrop);
        return () => window.removeEventListener(NODE_DROP_EVENT, handleDrop);
    }, [store, screenToFlowPosition, getNodes, setNodes, setEdges]);

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

        // left click places the nodes, like blender the click only confirms.
        // capture phase on pointerdown (fires before mousedown) and swallowed so react flow doesn't also
        // treat it as a click: no deselect on the pane, no selecting/dragging a node under the cursor.
        // right click is left alone so the context menu can cancel
        const handlePointerDown = (event: PointerEvent) => {
            if (event.button !== 0) return;
            event.stopPropagation();
            swallowClickRef.current = true;
            stopDragging();
        };

        window.addEventListener("mousemove", handleMouseMove);
        window.addEventListener("pointerdown", handlePointerDown, true);

        return () => {
            window.removeEventListener("mousemove", handleMouseMove);
            window.removeEventListener("pointerdown", handlePointerDown, true);
        };
    }, [newNodeToDrag, screenToFlowPosition, setNodes, stopDragging]);

    // swallow the rest of the placing click (mousedown/mouseup/click) so react flow never sees it.
    // lives outside the drag effect, which is torn down before the click event arrives
    useEffect(() => {
        const handleSwallow = (event: MouseEvent) => {
            if (!swallowClickRef.current) return;
            event.stopPropagation();
            if (event.type === "click") swallowClickRef.current = false;
        };
        // if the click never arrived (released outside the window), don't eat the next one
        const handlePointerDown = () => {
            swallowClickRef.current = false;
        };

        // registered before the drag effect's pointerdown, so a placing pointerdown sets the flag after this clears it
        window.addEventListener("pointerdown", handlePointerDown, true);
        window.addEventListener("mousedown", handleSwallow, true);
        window.addEventListener("mouseup", handleSwallow, true);
        window.addEventListener("click", handleSwallow, true);
        return () => {
            window.removeEventListener("pointerdown", handlePointerDown, true);
            window.removeEventListener("mousedown", handleSwallow, true);
            window.removeEventListener("mouseup", handleSwallow, true);
            window.removeEventListener("click", handleSwallow, true);
        };
    }, []);

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
                savePreOperationState();
                const { x, y } = mousePositionRef.current;
                setMenuPosition({ x, y });
                setMenuOpen(true);
            } else if (event.key === "Escape") {
                // cancel a grab/duplicate/add in progress, otherwise just close the menu
                if (preOperationStateRef.current) {
                    cancelOperation();
                } else {
                    closeMenu();
                }
            } else if (event.key.toLowerCase() === "x" && !event.metaKey && !event.ctrlKey) {
                event.preventDefault();
                // Delete selected nodes and edges
                // read from the store, the closure's nodes can be a render behind
                const selectedNodeIds = new Set(
                    getNodes()
                        .filter((n) => n.selected)
                        .map((n) => n.id)
                );
                const selectedEdgeIds = new Set(
                    getEdges()
                        .filter((e) => e.selected)
                        .map((e) => e.id)
                );
                if (selectedNodeIds.size === 0 && selectedEdgeIds.size === 0) return;

                // Delete selected nodes
                setNodes((nds) => nds.filter((node) => !selectedNodeIds.has(node.id)));

                // Delete selected edges and edges connected to deleted nodes
                setEdges((eds) => eds.filter((edge) => !selectedEdgeIds.has(edge.id) && !selectedNodeIds.has(edge.source) && !selectedNodeIds.has(edge.target)));

                setUpdateTrigger(true);
            } else if (event.key === "g") {
                event.preventDefault();
                const selectedNodes = getNodes().filter((node) => node.selected);
                if (selectedNodes.length > 0) {
                    savePreOperationState();
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
                // read from the store, the closure's nodes/edges can be a render behind
                const nodes = getNodes();
                const edges = getEdges();
                const selectedNodes = nodes.filter((node) => node.selected);
                if (selectedNodes.length === 0) return;

                savePreOperationState();

                const { x, y } = mousePositionRef.current;
                const flowPosition = screenToFlowPosition({ x, y }, { snapToGrid: false });

                // Create a map of old node IDs to new node IDs
                const oldToNewIdMap = new Map<string, string>();
                // ids already in use, pasted nodes get added as we go so they don't reuse the same id
                const takenIds: { id: string }[] = [...nodes];

                const newNodes = selectedNodes.map((node) => {
                    const newNodeId = nextNodeId(takenIds, node.type!);
                    takenIds.push({ id: newNodeId });
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

                // Add duplicated edges, only the duplicates stay selected
                setEdges((eds) => [...(eds ?? []).map((e) => (e.selected ? { ...e, selected: false } : e)), ...newEdges.map((e) => ({ ...e, selected: false }))]);

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
            } else if (event.key === "a") {
                event.preventDefault();

                // Check if any nodes are currently selected
                const hasSelection = getNodes().some((node) => node.selected) || getEdges().some((edge) => edge.selected);

                if (hasSelection) {
                    // If any are selected, deselect all
                    setNodes((nds) => nds.map((node) => ({ ...node, selected: false })));
                    setEdges((eds) => (eds ?? []).map((edge) => ({ ...edge, selected: false })));
                } else {
                    // If none are selected, select all
                    setNodes((nds) => nds.map((node) => ({ ...node, selected: true })));
                }
            }
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [screenToFlowPosition, setNodes, setEdges, getNodes, getEdges, savePreOperationState, cancelOperation, closeMenu]);

    // Close menu on click outside
    useEffect(() => {
        const handleClick = () => {
            if (menuOpen) {
                closeMenu();
            }
        };
        window.addEventListener("mousedown", handleClick);

        return () => window.removeEventListener("mousedown", handleClick);
    }, [menuOpen, closeMenu]);

    useEffect(() => {
        if ((updateTrigger || syncTrigger) && rfInstance) {
            let newState = { ...state, rf_instance: rfInstance?.toObject() };
            setState(newState);
            // only send the graph, the rest of our copy of the state may be stale
            invoke("js_update_graph", { rfInstance: JSON.stringify(newState.rf_instance) });

            // Block execution if paused, a plain sync (moving nodes) doesn't need a re-run
            if (updateTrigger && !state.execution_paused) {
                invoke("execute_graph", { realtime: true });
            }
            setUpdateTrigger(false);
            setSyncTrigger(false);
        }
    }, [rfInstance, updateTrigger, syncTrigger, state.execution_paused]);

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

    // shift multi select, tracked here instead of multiSelectionKeyCode.
    // react flow ignores a keyup inside an input, so releasing shift after shift+a focused the
    // add menu search left multi select stuck on (clicking another node added to the selection).
    // reading shiftKey off every key/mouse event means it can't get stuck
    useEffect(() => {
        const setMultiSelection = (active: boolean) => {
            if (store.getState().multiSelectionActive !== active) {
                store.setState({ multiSelectionActive: active });
            }
        };
        const handleEvent = (event: KeyboardEvent | MouseEvent) => setMultiSelection(event.shiftKey);
        const handleBlur = () => setMultiSelection(false);

        // capture phase so it's set before react flow handles the click
        window.addEventListener("keydown", handleEvent, true);
        window.addEventListener("keyup", handleEvent, true);
        window.addEventListener("pointerdown", handleEvent, true);
        window.addEventListener("mousedown", handleEvent, true);
        window.addEventListener("blur", handleBlur);
        return () => {
            window.removeEventListener("keydown", handleEvent, true);
            window.removeEventListener("keyup", handleEvent, true);
            window.removeEventListener("pointerdown", handleEvent, true);
            window.removeEventListener("mousedown", handleEvent, true);
            window.removeEventListener("blur", handleBlur);
        };
    }, [store]);

    // fit the view after a project load. not the fitView prop, that one stays armed on an empty graph and zooms onto the first node added
    const nodesInitialized = useNodesInitialized();
    const fitPendingRef = useRef(false);

    // a load asks for a fit, which waits until the new nodes have sizes
    useEffect(() => {
        const onLoaded = () => (fitPendingRef.current = true);
        window.addEventListener(PROJECT_LOADED_EVENT, onLoaded);
        return () => window.removeEventListener(PROJECT_LOADED_EVENT, onLoaded);
    }, []);

    useEffect(() => {
        if (fitPendingRef.current && nodesInitialized && rfInstance) {
            fitPendingRef.current = false;
            rfInstance.fitView({ maxZoom: 1 });
        }
    }, [nodesInitialized, rfInstance]);

    // Save & Load
    useEffect(() => {
        if (state.ready && state.rf_instance && rfInstance) {
            const storedGraph = state.rf_instance;

            // Check if the stored graph is different from current
            // selection and the viewport are UI only, the backend's copy of them is always stale
            const currentGraph = rfInstance.toObject();
            const isDifferent = graphKey(storedGraph) !== graphKey(currentGraph);

            if (isDifferent && storedGraph.nodes && storedGraph.edges) {
                // Reconstruct from loaded state
                // keep the UI only fields (measured size, selection) the backend doesn't track,
                // otherwise React Flow treats every node as unmeasured and hides it.
                // functional updates so we merge onto the latest nodes, not a snapshot from before
                // a click/drag that hasn't rendered yet (that's what left nodes stuck selected)
                setNodes((nds) => {
                    // nodes start out undefined before the first load
                    const currentNodes = new Map((nds ?? []).map((node: any) => [node.id, node]));
                    return storedGraph.nodes.map((node: any) => {
                        const prev: any = currentNodes.get(node.id);
                        if (!prev) return node;
                        // a node mid drag keeps its live position
                        const position = prev.dragging ? prev.position : node.position;
                        return { ...node, position, measured: node.measured ?? prev.measured, selected: prev.selected, dragging: prev.dragging };
                    });
                });
                setEdges((eds) => {
                    const currentEdges = new Map((eds ?? []).map((edge: any) => [edge.id, edge]));
                    return storedGraph.edges.map((edge: any) => {
                        const prev: any = currentEdges.get(edge.id);
                        return prev ? { ...edge, selected: prev.selected } : edge;
                    });
                });
            }
        }
    }, [state.rf_instance, state.ready, rfInstance]);

    // MARK: -
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

            // If shift key is not held, deselect all other nodes
            if (!event.shiftKey) {
                setNodes((nds) =>
                    nds.map((n) => ({
                        ...n,
                        selected: n.id === node.id,
                    }))
                );
            }
        },
        [newNodeToDrag, stopDragging, setNodes]
    );

    const handleNodeDrag = useCallback(
        (event: React.MouseEvent, node: any) => {
            if (newNodeToDrag) {
                stopDragging();
            }
        },
        [newNodeToDrag, stopDragging]
    );

    // Right Click to cancel
    const handleContextMenu = useCallback(
        (event: MouseEvent | React.MouseEvent<Element, MouseEvent>) => {
            event.preventDefault();
            cancelOperation();
        },
        [cancelOperation]
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

            setEdges((eds) => {
                // check if the connection is already present (target has an incoming edge)
                // looked up on the latest edges, not the ones from the last render
                const existingEdgeIndex = eds.findIndex((edge) => edge.source == params.source && edge.sourceHandle == params.sourceHandle);

                if (existingEdgeIndex !== -1) {
                    // if an edge exists to the target handle, replace it with the new connection
                    const updatedEdges = [...eds];
                    updatedEdges[existingEdgeIndex] = { ...params, id: updatedEdges[existingEdgeIndex].id };
                    return updatedEdges;
                }
                // if no existing edge, simply add the new connection
                return addEdge(params, eds);
            });

            setUpdateTrigger(true);
        },
        [setEdges]
    );

    const onNodesChange = useCallback(
        (changes: any) => {
            setNodes((nds) => applyNodeChanges(changes, nds));
            if (initDone.current && !state.ready && state.rf_instance == undefined && rfInstance == undefined) {
                return;
            }
            for (let change of changes) {
                if (change["type"] == "replace" || change["type"] == "remove") {
                    // update backend, node got replaced or deleted
                    setUpdateTrigger(true);
                } else if (change["type"] == "position" && change["dragging"] === false) {
                    // drag finished, keep the backend's positions current so a later push doesn't snap nodes back
                    setSyncTrigger(true);
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
            // only send the graph, the rest of our copy of the state may be stale
            invoke("js_update_graph", { rfInstance: JSON.stringify(newState.rf_instance) });
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
            <ReactFlow
                nodes={nodes}
                edges={edges}
                onNodesChange={onNodesChange}
                onEdgesChange={onEdgesChange}
                onConnect={onConnect}
                onPaneClick={handlePaneClick}
                onNodeClick={handleNodeClickStop}
                onNodeDrag={handleNodeDrag}
                onPaneContextMenu={handleContextMenu}
                onNodeContextMenu={handleContextMenu}
                onContextMenu={handleContextMenu}
                onSelectionContextMenu={handleContextMenu}
                onEdgeContextMenu={handleContextMenu}
                nodeTypes={nodeTypes}
                onInit={onInit}
                connectionLineComponent={ConnectionLine}
                isValidConnection={isValidConnection}
                panOnDrag={isPanningWithAlt ? true : [1]}
                selectionOnDrag={true}
                multiSelectionKeyCode={null}
                selectionKeyCode={"b"}
                selectionMode={SelectionMode.Partial}
                minZoom={0.05}
            >
                <Background variant={BackgroundVariant.Dots} gap={12} size={1} />
                <Controls />
                <MiniMap position="top-right" style={{ width: 100, height: 75 }} />
            </ReactFlow>
            <NodeAddMenu isOpen={menuOpen} onClose={closeMenu} onSelect={addNode} position={menuPosition} />
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
