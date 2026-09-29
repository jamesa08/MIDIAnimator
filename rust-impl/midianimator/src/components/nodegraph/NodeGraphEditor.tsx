import { listen } from "@tauri-apps/api/event";
import { useEffect, useState, useCallback, useRef, useMemo } from "react";

import { useNodesState, useEdgesState, addEdge, Connection, Edge, ReactFlowInstance, applyNodeChanges, applyEdgeChanges, useReactFlow, getOutgoers, useOnViewportChange, useStoreApi, useNodesInitialized, FinalConnectionState } from "@xyflow/react";
import { useStateContext } from "../../contexts/StateContext";
import { NODE_DROP_EVENT, PROJECT_LOADED_EVENT } from "../../utils/node";
import { FOR_EACH_INPUT, FOR_EACH_OUTPUT, GROUP, Graph, GroupDef, Level, NEW_SOCKET, Project, idPrefix, inputHandle, makeGroup, nextNodeId, outputHandle, socketId, specLookup, ungroup, withGraph, withGroup } from "../../utils/groups";
import { nodeEntries, useNodeSpecs } from "../../utils/nodeEntries";
import { SOCKET_EDIT_EVENT } from "../../nodes/_InterfaceNode";
import NodeGraphCanvas from "./NodeGraphCanvas";
import NodeAddMenu from "./NodeAddMenu";

// the parts of a graph the backend actually owns, used to tell if a backend push changed anything
function graphKey(graph: any): string {
    const nodes = (graph.nodes ?? []).map(({ selected, dragging, measured, ...node }: any) => node);
    const edges = (graph.edges ?? []).map(({ selected, ...edge }: any) => edge);
    return JSON.stringify({ nodes, edges });
}

// how the editor reads and writes the project, owned by NodeGraph
export type ProjectAccess = {
    // the latest project graph (root plus the project's groups)
    get: () => Project;
    // every group a group node can run, from the latest project
    groups: (project?: Project) => Record<string, GroupDef>;
    // sends a new project graph to the backend, `execute` re-runs the realtime graph
    commit: (project: Project, execute: boolean) => void;
};

type EditorProps = {
    // the graph this editor edits: the root, or the inside of a group
    level: Level;
    // group node ids from the root down to this graph
    path: string[];
    // the groups being run on the way down to this graph, adding one here would make it contain itself
    pathGroups: string[];
    // false inside a built-in group that hasn't been made local
    editable: boolean;
    project: ProjectAccess;
    specs: Record<string, any>;
    openGroup: (nodeId: string) => void;
    exitGroup: (toRoot: boolean) => void;
    // the nodes that were selected when this graph was last left, selected again when it opens
    initialSelection: Set<string>;
    // called once when the graph is left, with the nodes selected then
    onLeave: (selected: string[]) => void;
    // called once the graph has finished drawing (every node filled in, measured and framed), it's hidden until then
    onReady: () => void;
    // false while this graph is getting ready behind the one on screen, it doesn't take keys or drops then
    shown: boolean;
};

// edits one graph: selection, adding, grabbing, duplicating, deleting, connecting, grouping, and keeping the backend in sync.
// NodeGraph mounts a fresh one (in its own ReactFlowProvider) for each graph that's opened
function NodeGraphEditor({ level, path, pathGroups, editable, project, specs, openGroup, exitGroup, initialSelection, onLeave, onReady, shown }: EditorProps) {
    // start with the graph already there, a first sync must never see an empty graph (it would send it and wipe the project)
    const [nodes, setNodes] = useNodesState((level.graph.nodes ?? []).map(({ selected, dragging, measured, resizing, ...node }: any) => ({ ...node, selected: initialSelection.has(node.id) })));
    const [edges, setEdges] = useEdgesState((level.graph.edges ?? []).map(({ selected, ...edge }: any) => edge));

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

    const { backEndState: state } = useStateContext();
    const groupId = level.groupId;
    const isRoot = groupId === null;

    // read by the window listeners, a graph getting ready behind the one on screen ignores keys and drops
    const shownRef = useRef(shown);
    shownRef.current = shown;

    // set by the click that places nodes, the rest of that click gets swallowed
    const swallowClickRef = useRef(false);

    const dragStartRef = useRef<{ cursorX: number; cursorY: number; nodes: Array<{ id: string; x: number; y: number }> }>({
        cursorX: 0,
        cursorY: 0,
        nodes: [],
    });

    // the nodes that can be added here, groups on the way down to this graph are left out so none contains itself
    const groups = project.groups();
    const nodeSpecs = useNodeSpecs();
    const entries = useMemo(() => nodeEntries(groups, nodeSpecs, !isRoot, new Set(pathGroups)), [groups, nodeSpecs, isRoot, pathGroups]);

    // looks up a node's sockets (for types and names of new group sockets)
    const lookup = useMemo(() => specLookup(specs, groups), [specs, groups]);

    // edits in a read-only graph (an unedited built-in group) don't happen
    const canEdit = useCallback(() => editable, [editable]);

    // the current graph from React Flow's store
    const currentGraph = useCallback((): Graph => ({ ...(rfInstance?.toObject() ?? {}), nodes: getNodes(), edges: getEdges() }), [rfInstance, getNodes, getEdges]);

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

    // the nodes an add menu or nodes panel entry adds at `position`: one node, or both ends of a for each zone
    const entryNodes = useCallback(
        (key: string, position: { x: number; y: number }) => {
            const entry = entries.find((e) => e.key === key);
            if (!entry) return [];
            const taken: { id: string }[] = [...getNodes()];
            const make = (nodeType: string, data: any, offsetX: number) => {
                const id = nextNodeId(taken, idPrefix(nodeType, data));
                taken.push({ id });
                return { id, position: { x: position.x + offsetX, y: position.y }, data: { ...data }, type: nodeType, selected: true };
            };
            if (entry.nodeType === FOR_EACH_INPUT) {
                // a zone is always added as a pair, each end points at the other
                const input = make(FOR_EACH_INPUT, {}, 0);
                const output = make(FOR_EACH_OUTPUT, {}, 450);
                input.data.zone = output.id;
                output.data.zone = input.id;
                return [input, output];
            }
            return [make(entry.nodeType, entry.data, 0)];
        },
        [entries, getNodes]
    );

    // ADD NODE MENU HANDLERS
    // Add node creation function
    const addNode = useCallback(
        (key: string) => {
            if (!canEdit()) return setMenuOpen(false);
            const flowPosition = screenToFlowPosition(mousePositionRef.current, { snapToGrid: false });
            const newNodes = entryNodes(key, { x: flowPosition.x + 10, y: flowPosition.y + 10 });
            if (newNodes.length === 0) return setMenuOpen(false);

            // the new node becomes the only selection, like blender
            setNodes((nds) => [...nds.map((n) => (n.selected ? { ...n, selected: false } : n)), ...newNodes]);
            setEdges((eds) => (eds ?? []).map((e) => (e.selected ? { ...e, selected: false } : e)));

            // Store drag start for the new node
            dragStartRef.current = {
                cursorX: flowPosition.x - 10,
                cursorY: flowPosition.y - 10,
                nodes: newNodes.map((n) => ({ id: n.id, x: n.position.x - 10, y: n.position.y - 10 })),
            };

            setNewNodeToDrag(newNodes[0].id);
            setMenuOpen(false);
        },
        [setNodes, setEdges, entryNodes, screenToFlowPosition, canEdit]
    );
    // node dragged in from the nodes panel, added where it was released
    useEffect(() => {
        const handleDrop = (event: Event) => {
            if (!shownRef.current) return;
            const { nodeType, clientX, clientY, offsetX, offsetY } = (event as CustomEvent).detail;

            // only when released over the graph
            const rect = store.getState().domNode?.getBoundingClientRect();
            if (!rect || clientX < rect.left || clientX > rect.right || clientY < rect.top || clientY > rect.bottom) return;
            if (!canEdit()) return;

            // keep the node under the cursor where it was grabbed
            const flowPosition = screenToFlowPosition({ x: clientX, y: clientY }, { snapToGrid: false });
            const newNodes = entryNodes(nodeType, { x: flowPosition.x - offsetX, y: flowPosition.y - offsetY });
            if (newNodes.length === 0) return;

            // the new node becomes the only selection, like adding from the menu
            setNodes((nds) => [...(nds ?? []).map((n) => (n.selected ? { ...n, selected: false } : n)), ...newNodes]);
            setEdges((eds) => (eds ?? []).map((e) => (e.selected ? { ...e, selected: false } : e)));
            setSyncTrigger(true);
        };

        window.addEventListener(NODE_DROP_EVENT, handleDrop);
        return () => window.removeEventListener(NODE_DROP_EVENT, handleDrop);
    }, [store, screenToFlowPosition, entryNodes, setNodes, setEdges, canEdit]);

    // Track mouse position
    useEffect(() => {
        const handleMouseMove = (event: MouseEvent) => {
            mousePositionRef.current = { x: event.clientX, y: event.clientY };
        };

        window.addEventListener("mousemove", handleMouseMove);
        return () => window.removeEventListener("mousemove", handleMouseMove);
    }, []);

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

    // MARK: - Groups

    // Ctrl+G: the selected nodes become a new group, a group node takes their place
    const groupSelection = useCallback(() => {
        const selected = new Set(
            getNodes()
                .filter((n) => n.selected)
                .map((n) => n.id)
        );
        const current = project.get();
        const made = makeGroup(currentGraph(), selected, lookup, level.def, project.groups(current));
        if (!made) return;

        setNodes(made.graph.nodes);
        setEdges(made.graph.edges);
        // the new group and this graph's new layout in one update
        const withNewGroup = withGroup(current, made.groupId, made.def);
        project.commit(withGraph(withNewGroup, project.groups(withNewGroup), groupId, made.graph), true);
    }, [getNodes, project, currentGraph, lookup, level.def, groupId, setNodes, setEdges]);

    // Alt+G: the selected group nodes are replaced by the nodes inside their groups
    const ungroupSelection = useCallback(() => {
        const all = project.groups();
        const selectedGroups = getNodes().filter((n) => n.selected && n.type === GROUP && all[n.data?.group_id as string]);
        if (selectedGroups.length === 0) return;

        let graph = currentGraph();
        for (const node of selectedGroups) {
            graph = ungroup(graph, node.id, all[node.data.group_id as string]);
        }
        setNodes(graph.nodes);
        setEdges(graph.edges);
        setUpdateTrigger(true);
    }, [project, getNodes, currentGraph, setNodes, setEdges]);

    // renames or removes a socket of the group being edited (from the group input and output nodes)
    useEffect(() => {
        const handleSocketEdit = (event: Event) => {
            if (!shownRef.current) return;
            const { side, id, name } = (event as CustomEvent).detail as { side: "inputs" | "outputs"; id: string; name?: string };
            if (groupId === null || !canEdit()) return;
            const current = project.get();
            const def = project.groups(current)[groupId];
            if (!def) return;

            const sockets = name === undefined ? def.interface[side].filter((h) => h.id !== id) : def.interface[side].map((h) => (h.id === id ? { ...h, name } : h));
            let updated = withGraph(current, project.groups(current), groupId, currentGraph());
            updated = withGroup(updated, groupId, { ...project.groups(updated)[groupId], interface: { ...def.interface, [side]: sockets } });

            if (name === undefined) {
                // a removed socket takes its connections with it: inside on the group input/output, outside on every group node running this group
                const inside = side === "inputs" ? "group_input" : "group_output";
                const dropEdges = (graph: Graph, isInside: boolean) => {
                    const byId = new Map(graph.nodes.map((n: any) => [n.id, n]));
                    return graph.edges.filter((e: any) => {
                        if (isInside) {
                            const node = side === "inputs" ? byId.get(e.target) : byId.get(e.source);
                            const handle = side === "inputs" ? e.targetHandle : e.sourceHandle;
                            return !(node?.type === inside && handle === id);
                        }
                        const node = side === "inputs" ? byId.get(e.source) : byId.get(e.target);
                        const handle = side === "inputs" ? e.sourceHandle : e.targetHandle;
                        return !(node?.type === GROUP && node.data?.group_id === groupId && handle === id);
                    });
                };
                updated = { ...updated, edges: dropEdges(updated, false) };
                for (const [gid, g] of Object.entries(updated.groups ?? {})) {
                    const edges = dropEdges(g, gid === groupId);
                    if (edges.length !== g.edges.length) updated = withGroup(updated, gid, { ...g, edges });
                }
                setEdges(updated.groups![groupId].edges);
            }
            project.commit(updated, true);
        };
        window.addEventListener(SOCKET_EDIT_EVENT, handleSocketEdit);
        return () => window.removeEventListener(SOCKET_EDIT_EVENT, handleSocketEdit);
    }, [groupId, project, currentGraph, setEdges, canEdit]);

    // Keyboard listener
    useEffect(() => {
        const handleKeyDown = (event: KeyboardEvent) => {
            if (!shownRef.current) return;
            // ignore if focused on input and not keybind
            const target = event.target as HTMLElement;
            if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable) {
                return;
            }

            if (event.key === "Tab") {
                // tab opens the selected group, or goes back out when no group is selected. ctrl+tab goes back to the top
                event.preventDefault();
                cancelOperation();
                const selectedGroups = getNodes().filter((n) => n.selected && n.type === GROUP);
                if (event.ctrlKey) {
                    exitGroup(true);
                } else if (selectedGroups.length === 1) {
                    openGroup(selectedGroups[0].id);
                } else {
                    exitGroup(false);
                }
            } else if (event.code === "KeyG" && (event.ctrlKey || event.metaKey) && !event.altKey) {
                event.preventDefault();
                if (canEdit()) groupSelection();
            } else if (event.code === "KeyG" && event.altKey) {
                event.preventDefault();
                if (canEdit()) ungroupSelection();
            } else if (event.shiftKey && event.key === "A") {
                event.preventDefault();
                if (!canEdit()) return;
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
                if (!canEdit()) return;
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
                // a zone is deleted as a pair, half a zone can't run
                for (const node of getNodes()) {
                    if (selectedNodeIds.has(node.id) && node.data?.zone) selectedNodeIds.add(node.data.zone as string);
                }

                // Delete selected nodes
                setNodes((nds) => nds.filter((node) => !selectedNodeIds.has(node.id)));

                // Delete selected edges and edges connected to deleted nodes
                setEdges((eds) => eds.filter((edge) => !selectedEdgeIds.has(edge.id) && !selectedNodeIds.has(edge.source) && !selectedNodeIds.has(edge.target)));

                setUpdateTrigger(true);
            } else if (event.key === "g" && !event.ctrlKey && !event.metaKey && !event.altKey) {
                event.preventDefault();
                if (!editable) return;
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
                if (!canEdit()) return;
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
                    const newNodeId = nextNodeId(takenIds, idPrefix(node.type!, node.data));
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
                // duplicated zone ends point at each other's copies, a zone end copied alone isn't paired
                for (const node of newNodes) {
                    if (!node.data.zone) continue;
                    const pair = oldToNewIdMap.get(node.data.zone as string);
                    if (pair) node.data.zone = pair;
                    else delete node.data.zone;
                }

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
    }, [screenToFlowPosition, setNodes, setEdges, getNodes, getEdges, savePreOperationState, cancelOperation, closeMenu, openGroup, exitGroup, groupSelection, ungroupSelection, editable, canEdit]);

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

    // send this graph to the backend, a read-only group is never written (that would make it local)
    useEffect(() => {
        if ((updateTrigger || syncTrigger) && rfInstance) {
            if (editable) {
                const current = project.get();
                project.commit(withGraph(current, project.groups(current), groupId, rfInstance.toObject()), updateTrigger);
            }
            setUpdateTrigger(false);
            setSyncTrigger(false);
        }
    }, [rfInstance, updateTrigger, syncTrigger, editable, groupId, project]);

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

    // fit the view after a project load, or when a group without a saved view is opened.
    // not the fitView prop, that one stays armed on an empty graph and zooms onto the first node added
    const nodesInitialized = useNodesInitialized();
    const fitPendingRef = useRef(!isRoot && !level.graph.viewport);

    // a loaded project starts the graph over, nothing from the old graph carries over.
    // short ids repeat between projects, so merging would keep the old selection, drags and operations on the new nodes.
    // only the top level listens, NodeGraph goes back to it on a load
    useEffect(() => {
        if (!isRoot) return;
        const unlisten = listen(PROJECT_LOADED_EVENT, (event: any) => {
            const graph = event.payload ?? {};

            // drop any operation in progress (grab, add menu, placing a node)
            dragStartRef.current = { cursorX: 0, cursorY: 0, nodes: [] };
            preOperationStateRef.current = null;
            swallowClickRef.current = false;
            setNewNodeToDrag(null);
            setMenuOpen(false);
            setUpdateTrigger(false);
            setSyncTrigger(false);

            // replace the graph without the ui only fields, measured is left out so the nodes get sized fresh
            setNodes((graph.nodes ?? []).map(({ selected, dragging, measured, resizing, ...node }: any) => node));
            setEdges((graph.edges ?? []).map(({ selected, ...edge }: any) => edge));
            store.setState({ nodesSelectionActive: false });

            // fit once the new nodes have sizes
            fitPendingRef.current = true;
        });
        return () => {
            unlisten.then((f) => f());
        };
    }, [isRoot, setNodes, setEdges, store]);

    // hold the graph hidden until it has finished drawing: every node filled in (a node's spec loads after its first
    // frame), sizes that stayed the same for two frames, and the view framed. a second at most, it never stays hidden
    const [ready, setReady] = useState(false);
    useEffect(() => {
        if (ready || !rfInstance) return;
        let done = false;
        let last = "";
        let steady = 0;
        let frame = 0;
        const finish = () => {
            if (done) return;
            done = true;
            if (fitPendingRef.current && getNodes().length > 0) {
                fitPendingRef.current = false;
                rfInstance.fitView({ maxZoom: 1 });
            }
            // one more frame so the framed view is what shows first
            frame = requestAnimationFrame(() => {
                setReady(true);
                onReady();
            });
        };
        const tick = () => {
            const nodes = getNodes();
            const sizes = nodes.map((n) => `${n.measured?.width}x${n.measured?.height}`).join(",");
            const empty = store.getState().domNode?.querySelectorAll(".react-flow__node .node-header > span:first-child:empty").length ?? 0;
            const settled = nodes.every((n) => n.measured?.width) && empty === 0 && sizes === last;
            steady = settled ? steady + 1 : 0;
            last = sizes;
            if (steady >= 2) finish();
            else if (!done) frame = requestAnimationFrame(tick);
        };
        frame = requestAnimationFrame(tick);
        // animation frames pause while the window is in the background, a timer makes sure it's shown
        const cap = setTimeout(() => {
            finish();
            if (!document.hasFocus()) {
                setReady(true);
                onReady();
            }
        }, 1000);
        return () => {
            cancelAnimationFrame(frame);
            clearTimeout(cap);
        };
    }, [ready, rfInstance, getNodes, store, onReady]);

    // hand the selection back when this graph is left, so it's there again when it's opened next
    const onLeaveRef = useRef(onLeave);
    onLeaveRef.current = onLeave;
    useEffect(
        () => () =>
            onLeaveRef.current(
                getNodes()
                    .filter((n) => n.selected)
                    .map((n) => n.id)
            ),
        [getNodes]
    );

    useEffect(() => {
        // a project load while the graph is up (framing on the first draw is done above)
        if (ready && fitPendingRef.current && nodesInitialized && rfInstance && getNodes().length > 0) {
            fitPendingRef.current = false;
            rfInstance.fitView({ maxZoom: 1 });
        }
    }, [ready, nodesInitialized, rfInstance, getNodes]);

    // Save & Load
    // the stored graph changed (backend push, e.g. an MCP edit, or this editor just opened): take it
    const stored = level.graph;
    useEffect(() => {
        if (state.ready && stored && rfInstance) {
            // Check if the stored graph is different from current
            // selection and the viewport are UI only, the backend's copy of them is always stale
            const currentGraph = rfInstance.toObject();
            const isDifferent = graphKey(stored) !== graphKey(currentGraph);

            if (isDifferent && stored.nodes && stored.edges) {
                // Reconstruct from loaded state
                // keep the UI only fields (measured size, selection) the backend doesn't track,
                // otherwise React Flow treats every node as unmeasured and hides it.
                // functional updates so we merge onto the latest nodes, not a snapshot from before
                // a click/drag that hasn't rendered yet (that's what left nodes stuck selected)
                setNodes((nds) => {
                    // nodes start out undefined before the first load
                    const currentNodes = new Map((nds ?? []).map((node: any) => [node.id, node]));
                    return stored.nodes.map((node: any) => {
                        const prev: any = currentNodes.get(node.id);
                        if (!prev) return node;
                        // a node mid drag keeps its live position
                        const position = prev.dragging ? prev.position : node.position;
                        return { ...node, position, measured: node.measured ?? prev.measured, selected: prev.selected, dragging: prev.dragging };
                    });
                });
                setEdges((eds) => {
                    const currentEdges = new Map((eds ?? []).map((edge: any) => [edge.id, edge]));
                    return stored.edges.map((edge: any) => {
                        const prev: any = currentEdges.get(edge.id);
                        return prev ? { ...edge, selected: prev.selected } : edge;
                    });
                });
            }
        }
    }, [stored, state.ready, rfInstance]);

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
        // keep where the graph is looked at, coming back out of a group restores it
        onEnd: () => setSyncTrigger(true),
    });

    // connecting to the empty socket on the group input or output adds a socket to the group, named and typed after the other end
    const addGroupSocket = useCallback(
        (params: Connection): Connection | null => {
            if (groupId === null) return null;
            const nodes = getNodes();
            const current = project.get();
            const def = project.groups(current)[groupId];
            if (!def) return null;

            // the group input's empty socket is an output (target), the group output's an input (source)
            const toInputs = params.targetHandle === NEW_SOCKET;
            const other = nodes.find((n) => n.id === (toInputs ? params.source : params.target));
            const handle = toInputs ? inputHandle(lookup, other, params.sourceHandle!, def) : outputHandle(lookup, other, params.targetHandle!, def);
            const side = toInputs ? "inputs" : "outputs";
            const id = socketId(handle.name, def.interface[side]);
            const sockets = [...def.interface[side], { id, name: handle.name, data_type: handle.data_type, description: handle.description ?? "" }];

            // the interface change is sent now, the new edge goes with the graph right after
            const updated = withGraph(current, project.groups(current), groupId, currentGraph());
            project.commit(withGroup(updated, groupId, { ...project.groups(updated)[groupId], interface: { ...def.interface, [side]: sockets } }), false);
            return toInputs ? { ...params, targetHandle: id } : { ...params, sourceHandle: id };
        },
        [groupId, getNodes, project, lookup, currentGraph]
    );

    const onConnect = useCallback(
        (connection: Edge | Connection) => {
            if (!editable) return;
            let params = connection;
            if (params.sourceHandle === NEW_SOCKET || params.targetHandle === NEW_SOCKET) {
                const added = addGroupSocket(params as Connection);
                if (!added) return;
                params = added;
            }

            setEdges((eds) => {
                // check if the connection is already present (target has an incoming edge)
                // looked up on the latest edges, not the ones from the last render
                const existingEdgeIndex = eds.findIndex((edge) => edge.source == params.source && edge.sourceHandle == params.sourceHandle);

                if (existingEdgeIndex !== -1) {
                    // if an edge exists to the target handle, replace it with the new connection
                    const updatedEdges = [...eds];
                    updatedEdges[existingEdgeIndex] = { ...params, id: updatedEdges[existingEdgeIndex].id } as Edge;
                    return updatedEdges;
                }
                // if no existing edge, simply add the new connection
                return addEdge(params, eds);
            });

            setUpdateTrigger(true);
        },
        [setEdges, editable, addGroupSocket]
    );

    // link dragged off a handle and dropped on nothing (the + sign), open the add menu where it was released
    const onConnectEnd = useCallback(
        (event: MouseEvent | TouchEvent, connectionState: FinalConnectionState) => {
            if (connectionState.isValid || connectionState.toHandle || !editable) return;

            const { clientX, clientY } = "changedTouches" in event ? event.changedTouches[0] : event;
            // the new node gets placed from this position, same as shift+a
            mousePositionRef.current = { x: clientX, y: clientY };
            savePreOperationState();
            setMenuPosition({ x: clientX, y: clientY });
            setMenuOpen(true);
        },
        [savePreOperationState, editable]
    );

    const onNodesChange = useCallback(
        (changes: any) => {
            setNodes((nds) => applyNodeChanges(changes, nds));
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
        [setNodes]
    );

    const onEdgesChange = useCallback(
        (changes: any) => {
            setEdges((eds) => applyEdgeChanges(changes, eds));
            for (let change of changes) {
                if (change["type"] == "remove") {
                    // update backend, node got replaced
                    setUpdateTrigger(true);
                }
            }
        },
        [setEdges]
    );

    const onInit = useCallback(
        (instance: ReactFlowInstance) => {
            setRfInstance(instance);
            // a graph opens where it was last looked at
            if (level.graph.viewport) instance.setViewport(level.graph.viewport);
        },
        [isRoot, level.graph.viewport]
    );

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
            <NodeGraphCanvas
                nodes={nodes}
                edges={edges}
                onNodesChange={onNodesChange}
                onEdgesChange={onEdgesChange}
                onConnect={onConnect}
                onConnectEnd={onConnectEnd}
                onPaneClick={handlePaneClick}
                onNodeClick={handleNodeClickStop}
                onNodeDrag={handleNodeDrag}
                onPaneContextMenu={handleContextMenu}
                onNodeContextMenu={handleContextMenu}
                onContextMenu={handleContextMenu}
                onSelectionContextMenu={handleContextMenu}
                onEdgeContextMenu={handleContextMenu}
                onInit={onInit}
                isValidConnection={isValidConnection}
                panOnDrag={isPanningWithAlt ? true : [1]}
                nodesDraggable={editable}
                nodesConnectable={editable}
                className={isRoot ? "" : "group-canvas"}
            />
            <NodeAddMenu isOpen={menuOpen} entries={entries} onClose={closeMenu} onSelect={addNode} position={menuPosition} />
        </>
    );
}

export default NodeGraphEditor;
