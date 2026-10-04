import { useEffect, useState, useCallback, useRef, useMemo } from "react";

import { useNodesState, useEdgesState, Connection, Edge, ReactFlowInstance, applyNodeChanges, applyEdgeChanges, useReactFlow, getOutgoers, useOnViewportChange, useStoreApi, useNodesInitialized, FinalConnectionState } from "@xyflow/react";
import { useStateContext } from "../../contexts/StateContext";
import { NODE_DROP_EVENT } from "../../utils/node";
import { GROUP, GroupDef, Level, Project } from "../../utils/groups";
import { ApplyOptions, Op, useGraphOps } from "../../utils/graphOps";
import { isTextField } from "../../utils/editMenu";
import { nodeEntries, useNodeSpecs } from "../../utils/nodeEntries";
import { SOCKET_EDIT_EVENT } from "../../nodes/_InterfaceNode";
import NodeGraphCanvas from "./NodeGraphCanvas";
import NodeAddMenu from "./NodeAddMenu";

// how the editor reads the project, owned by NodeGraph. edits go to the backend as ops (utils/graphOps.ts)
export type ProjectAccess = {
    // the latest project graph (root plus the project's groups)
    get: () => Project;
    // every group a group node can run, from the latest project
    groups: (project?: Project) => Record<string, GroupDef>;
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
    openGroup: (nodeId: string) => void;
    exitGroup: (toRoot: boolean) => void;
    // called once the graph has finished drawing (every node filled in, measured and framed), it's hidden until then
    onReady: () => void;
    // false while this graph is getting ready behind the one on screen, it doesn't take keys or drops then
    shown: boolean;
    // frame the graph to fit instead of opening where it was last looked at (a loaded project's tab)
    fitOnOpen: boolean;
};

// nodes following the cursor until a click places them (G, Shift+D, adding from the menu). `txn` is the transaction the
// add or duplicate was made in, placing ends it and cancelling undoes it
type Grab = { txn: string | null; cursorX: number; cursorY: number; nodes: { id: string; x: number; y: number }[] };

// ids of the selected nodes or edges
const selectedIds = (items: { id: string; selected?: boolean }[]) => items.filter((i) => i.selected).map((i) => i.id);

// edits one graph: selection, adding, grabbing, duplicating, deleting, connecting, grouping. every edit is sent to the
// backend as an op and the graph it sends back is shown, only selection and positions mid drag are ahead of it.
// NodeGraph mounts a fresh one (in its own ReactFlowProvider) for each graph that's opened
function NodeGraphEditor({ level, path, pathGroups, editable, project, openGroup, exitGroup, onReady, shown, fitOnOpen }: EditorProps) {
    // start with the graph already there, the selection is part of it
    const [nodes, setNodes] = useNodesState((level.graph.nodes ?? []).map(({ dragging, measured, resizing, ...node }: any) => node));
    const [edges, setEdges] = useEdgesState(level.graph.edges ?? []);

    const { getNodes, getEdges, screenToFlowPosition } = useReactFlow();
    const store = useStoreApi();

    const [rfInstance, setRfInstance] = useState(null as ReactFlowInstance | null);

    const [menuOpen, setMenuOpen] = useState(false);
    const [menuPosition, setMenuPosition] = useState({ x: 0, y: 0 });
    const mousePositionRef = useRef({ x: 0, y: 0 });

    // the grab in progress, `grabbing` re-renders when one starts or ends
    const grabRef = useRef<Grab | null>(null);
    const [grabbing, setGrabbing] = useState(false);

    //emulate 3 button mouse panning
    const [isPanningWithAlt, setIsPanningWithAlt] = useState(false);

    const { backEndState: state } = useStateContext();
    const groupId = level.groupId;
    const isRoot = groupId === null;
    const ops = useGraphOps(groupId);

    // read by the window listeners, a graph getting ready behind the one on screen ignores keys and drops
    const shownRef = useRef(shown);
    shownRef.current = shown;

    // set by the click that places nodes, the rest of that click gets swallowed
    const swallowClickRef = useRef(false);

    // ops sent (or a selection about to be sent) that haven't come back yet, the selection on screen is ahead of the
    // backend's until they have
    const inFlightRef = useRef(0);
    // the backend's latest graph, for callbacks
    const storedRef = useRef(level.graph);
    storedRef.current = level.graph;

    // the nodes that can be added here, groups on the way down to this graph are left out so none contains itself
    const groups = project.groups();
    const nodeSpecs = useNodeSpecs();
    const entries = useMemo(() => nodeEntries(groups, nodeSpecs, !isRoot, new Set(pathGroups)), [groups, nodeSpecs, isRoot, pathGroups]);

    // edits in a read-only graph (an unedited built-in group) don't happen
    const canEdit = useCallback(() => editable, [editable]);

    // an edit on its way to the backend, a failed one changed nothing
    const track = useCallback(async <T,>(label: string, run: () => Promise<T>) => {
        inFlightRef.current++;
        try {
            return await run();
        } catch (e) {
            console.error(`${label}: ${e}`);
            return null;
        } finally {
            inFlightRef.current--;
        }
    }, []);

    // sends ops to the backend
    const apply = useCallback((list: Op[], options: ApplyOptions = {}) => track(list.map((o) => o.op).join(", "), () => ops.apply(list, options)), [ops, track]);

    // MARK: - Selection

    // the selection on screen goes to the backend once per frame (a click selects and deselects in several changes),
    // after a box selection has ended. selecting is an undo step
    const selectionPendingRef = useRef(false);
    const scheduleSelection = useCallback(() => {
        if (selectionPendingRef.current) return;
        selectionPendingRef.current = true;
        inFlightRef.current++;
        const flush = () => {
            if (store.getState().userSelectionActive) return requestAnimationFrame(flush);
            selectionPendingRef.current = false;
            const nodes = selectedIds(getNodes());
            const edges = selectedIds(getEdges());
            const stored = storedRef.current;
            const same = (a: string[], b: string[]) => a.length === b.length && a.every((id) => b.includes(id));
            if (canEdit() && !(same(nodes, selectedIds(stored.nodes ?? [])) && same(edges, selectedIds(stored.edges ?? [])))) {
                apply([{ op: "select", nodes, edges }]);
            }
            inFlightRef.current--;
        };
        requestAnimationFrame(flush);
    }, [store, getNodes, getEdges, canEdit, apply]);

    // MARK: - Grab

    // the nodes follow the cursor from `cursor` (flow position) until a click places them
    const startGrab = useCallback((grabbed: { id: string; position: { x: number; y: number } }[], cursor: { x: number; y: number }, txn: string | null) => {
        grabRef.current = { txn, cursorX: cursor.x, cursorY: cursor.y, nodes: grabbed.map((n) => ({ id: n.id, x: n.position.x, y: n.position.y })) };
        setGrabbing(true);
    }, []);

    // places the grabbed nodes where they are: one move, ending the add or duplicate it belongs to
    const confirmGrab = useCallback(() => {
        const grab = grabRef.current;
        grabRef.current = null;
        setGrabbing(false);
        if (!grab) return;
        const placed = new Map(getNodes().map((n) => [n.id, n.position]));
        const positions = Object.fromEntries(grab.nodes.filter((n) => placed.has(n.id)).map((n) => [n.id, placed.get(n.id)!]));
        apply([{ op: "move", positions }], grab.txn ? { txn: grab.txn } : {}).then(() => grab.txn && ops.end(grab.txn));
    }, [getNodes, apply, ops]);

    // puts the grabbed nodes back, an add or duplicate is undone
    const cancelGrab = useCallback(() => {
        const grab = grabRef.current;
        grabRef.current = null;
        setGrabbing(false);
        setMenuOpen(false);
        if (!grab) return;
        if (grab.txn) {
            ops.cancel(grab.txn);
        } else {
            const start = new Map(grab.nodes.map((n) => [n.id, { x: n.x, y: n.y }]));
            setNodes((nds) => nds.map((node) => (start.has(node.id) ? { ...node, position: start.get(node.id)! } : node)));
        }
    }, [ops, setNodes]);

    // leaving the graph mid grab (another tab or group is shown) undoes the add or duplicate it belongs to
    const opsRef = useRef(ops);
    opsRef.current = ops;
    useEffect(
        () => () => {
            const txn = grabRef.current?.txn;
            if (txn) opsRef.current.cancel(txn);
        },
        []
    );

    // close the add menu without adding anything
    const closeMenu = useCallback(() => setMenuOpen(false), []);

    // ADD NODE MENU HANDLERS
    // the node (a for each zone brings its other end) is added under the cursor and grabbed until a click places it
    const addNode = useCallback(
        async (key: string) => {
            setMenuOpen(false);
            const entry = entries.find((e) => e.key === key);
            if (!entry || !canEdit()) return;
            const flowPosition = screenToFlowPosition(mousePositionRef.current, { snapToGrid: false });
            const txn = crypto.randomUUID();
            const applied = await apply([{ op: "add_nodes", nodes: [{ type: entry.nodeType, data: entry.data, position: { x: flowPosition.x + 10, y: flowPosition.y + 10 } }] }], { txn });
            if (applied) startGrab(applied.added, flowPosition, txn);
        },
        [entries, canEdit, screenToFlowPosition, apply, startGrab]
    );
    // node dragged in from the nodes panel, added where it was released
    useEffect(() => {
        const handleDrop = (event: Event) => {
            if (!shownRef.current) return;
            const { nodeType, clientX, clientY, offsetX, offsetY } = (event as CustomEvent).detail;

            // only when released over the graph
            const rect = store.getState().domNode?.getBoundingClientRect();
            if (!rect || clientX < rect.left || clientX > rect.right || clientY < rect.top || clientY > rect.bottom) return;
            // docked panels float over the graph, a node released on one isn't added under it
            if (document.elementFromPoint(clientX, clientY)?.closest(".panel")) return;
            const entry = entries.find((e) => e.key === nodeType);
            if (!entry || !canEdit()) return;

            // keep the node under the cursor where it was grabbed
            const flowPosition = screenToFlowPosition({ x: clientX, y: clientY }, { snapToGrid: false });
            apply([{ op: "add_nodes", nodes: [{ type: entry.nodeType, data: entry.data, position: { x: flowPosition.x - offsetX, y: flowPosition.y - offsetY } }] }]);
        };

        window.addEventListener(NODE_DROP_EVENT, handleDrop);
        return () => window.removeEventListener(NODE_DROP_EVENT, handleDrop);
    }, [store, screenToFlowPosition, entries, canEdit, apply]);

    // Track mouse position
    useEffect(() => {
        const handleMouseMove = (event: MouseEvent) => {
            mousePositionRef.current = { x: event.clientX, y: event.clientY };
        };

        window.addEventListener("mousemove", handleMouseMove);
        return () => window.removeEventListener("mousemove", handleMouseMove);
    }, []);

    useEffect(() => {
        if (!grabbing) return;

        const handleMouseMove = (event: MouseEvent) => {
            const grab = grabRef.current;
            if (!grab) return;
            const flowPosition = screenToFlowPosition({ x: event.clientX, y: event.clientY }, { snapToGrid: false });

            // Calculate delta from start position
            const dx = flowPosition.x - grab.cursorX;
            const dy = flowPosition.y - grab.cursorY;
            const start = new Map(grab.nodes.map((n) => [n.id, n]));

            setNodes((nds) => nds.map((node) => (start.has(node.id) ? { ...node, position: { x: start.get(node.id)!.x + dx, y: start.get(node.id)!.y + dy } } : node)));
        };

        // left click places the nodes, like blender the click only confirms.
        // capture phase on pointerdown (fires before mousedown) and swallowed so react flow doesn't also
        // treat it as a click: no deselect on the pane, no selecting/dragging a node under the cursor.
        // right click is left alone so the context menu can cancel
        const handlePointerDown = (event: PointerEvent) => {
            if (event.button !== 0) return;
            event.stopPropagation();
            swallowClickRef.current = true;
            confirmGrab();
        };

        window.addEventListener("mousemove", handleMouseMove);
        window.addEventListener("pointerdown", handlePointerDown, true);

        return () => {
            window.removeEventListener("mousemove", handleMouseMove);
            window.removeEventListener("pointerdown", handlePointerDown, true);
        };
    }, [grabbing, screenToFlowPosition, setNodes, confirmGrab]);

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

    // MARK: - Clipboard

    // copy, cut and paste (the edit menu or its shortcuts) arrive as the page's clipboard events, never as keys (both fire).
    // a text field, or text selected on the page, keeps its own. the graph's go to the backend, which uses the system clipboard
    const lastPasteRef = useRef<{ x: number; y: number; count: number } | null>(null);
    useEffect(() => {
        const ownsClipboard = () => shownRef.current && !isTextField(document.activeElement) && !window.getSelection()?.toString();

        const handleCopy = (event: ClipboardEvent) => {
            const nodes = selectedIds(getNodes());
            if (!ownsClipboard() || nodes.length === 0) return;
            event.preventDefault();
            ops.copy(nodes).catch((e) => console.error(`copy: ${e}`));
        };

        const handleCut = (event: ClipboardEvent) => {
            const nodes = selectedIds(getNodes());
            const edges = selectedIds(getEdges());
            if (!ownsClipboard() || !canEdit() || (nodes.length === 0 && edges.length === 0)) return;
            event.preventDefault();
            track("cut", () => ops.cut(nodes, edges));
        };

        // under the cursor when it's over the graph, otherwise in the middle of the view. pasting again without moving
        // the cursor steps each paste down and to the right
        const handlePaste = (event: ClipboardEvent) => {
            const rect = store.getState().domNode?.getBoundingClientRect();
            if (!ownsClipboard() || !canEdit() || !rect) return;
            event.preventDefault();
            const { x, y } = mousePositionRef.current;
            const over = x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
            const at = screenToFlowPosition(over ? { x, y } : { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }, { snapToGrid: false });
            const last = lastPasteRef.current;
            const count = last && Math.abs(last.x - at.x) < 1 && Math.abs(last.y - at.y) < 1 ? last.count + 1 : 0;
            lastPasteRef.current = { ...at, count };
            track("paste", () => ops.paste({ x: at.x + 20 * count, y: at.y + 20 * count }));
        };

        // webkit enables the edit menu's copy, cut and paste for a page that claims these. copied nodes aren't text, so
        // without it paste stays off
        const claim = (event: Event) => {
            if (ownsClipboard() && (event.type !== "beforecopy" || selectedIds(getNodes()).length > 0)) event.preventDefault();
        };

        const listeners = { copy: handleCopy, cut: handleCut, paste: handlePaste, beforecopy: claim, beforecut: claim, beforepaste: claim } as Record<string, (event: any) => void>;
        for (const [type, listener] of Object.entries(listeners)) document.addEventListener(type, listener);
        return () => {
            for (const [type, listener] of Object.entries(listeners)) document.removeEventListener(type, listener);
        };
    }, [getNodes, getEdges, canEdit, ops, track, store, screenToFlowPosition]);

    // MARK: - Groups

    // Ctrl+G: the selected nodes become a new group, a group node takes their place
    const groupSelection = useCallback(() => {
        const selected = getNodes().filter((n) => n.selected);
        if (selected.length === 0) return;
        const widths = Object.fromEntries(selected.filter((n) => n.measured?.width).map((n) => [n.id, n.measured!.width!]));
        apply([{ op: "group", nodes: selected.map((n) => n.id), widths }]);
    }, [getNodes, apply]);

    // Alt+G: the selected group nodes are replaced by the nodes inside their groups
    const ungroupSelection = useCallback(() => {
        const selected = getNodes().filter((n) => n.selected && n.type === GROUP);
        if (selected.length === 0) return;
        apply([{ op: "ungroup", nodes: selected.map((n) => n.id) }]);
    }, [getNodes, apply]);

    // renames or removes a socket of the group being edited (from the group input and output nodes)
    useEffect(() => {
        const handleSocketEdit = (event: Event) => {
            if (!shownRef.current) return;
            const { side, id, name } = (event as CustomEvent).detail as { side: "inputs" | "outputs"; id: string; name?: string };
            if (groupId === null || !canEdit()) return;
            apply([name === undefined ? { op: "remove_socket", side, id } : { op: "rename_socket", side, id, name }]);
        };
        window.addEventListener(SOCKET_EDIT_EVENT, handleSocketEdit);
        return () => window.removeEventListener(SOCKET_EDIT_EVENT, handleSocketEdit);
    }, [groupId, canEdit, apply]);

    // Keyboard listener
    useEffect(() => {
        const handleKeyDown = async (event: KeyboardEvent) => {
            if (!shownRef.current) return;
            // ignore if focused on input and not keybind
            const target = event.target as HTMLElement;
            if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable) {
                return;
            }

            if (event.key === "Tab") {
                // tab opens the selected group, or goes back out when no group is selected. ctrl+tab goes back to the top
                event.preventDefault();
                cancelGrab();
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
                const { x, y } = mousePositionRef.current;
                setMenuPosition({ x, y });
                setMenuOpen(true);
            } else if (event.key === "Escape") {
                // cancel a grab in progress, otherwise just close the menu
                if (grabRef.current) {
                    cancelGrab();
                } else {
                    closeMenu();
                }
            } else if (event.key.toLowerCase() === "x" && !event.metaKey && !event.ctrlKey) {
                event.preventDefault();
                if (!canEdit()) return;
                // the selected nodes (a zone as a pair) and edges, edges of deleted nodes go with them
                // read from the store, the closure's nodes can be a render behind
                const nodes = selectedIds(getNodes());
                const edges = selectedIds(getEdges());
                if (nodes.length === 0 && edges.length === 0) return;
                apply([{ op: "delete", nodes, edges }]);
            } else if (event.key === "g" && !event.ctrlKey && !event.metaKey && !event.altKey) {
                event.preventDefault();
                if (!editable) return;
                const selected = getNodes().filter((node) => node.selected);
                if (selected.length > 0) {
                    const { x, y } = mousePositionRef.current;
                    startGrab(selected, screenToFlowPosition({ x, y }, { snapToGrid: false }), null);
                }
            } else if (event.shiftKey && event.key === "D") {
                event.preventDefault();
                if (!canEdit()) return;
                // read from the store, the closure's nodes can be a render behind
                const selected = selectedIds(getNodes());
                if (selected.length === 0) return;

                // the copies are grabbed, placing them ends the duplicate and cancelling undoes it
                const { x, y } = mousePositionRef.current;
                const flowPosition = screenToFlowPosition({ x, y }, { snapToGrid: false });
                const txn = crypto.randomUUID();
                const applied = await apply([{ op: "duplicate", nodes: selected, offset: { x: 20, y: 20 } }], { txn });
                if (applied) startGrab(applied.added, flowPosition, txn);
            } else if (event.key === "a" && !event.metaKey && !event.ctrlKey) {
                event.preventDefault();

                // deselect everything if anything is selected, otherwise select every node
                const hasSelection = getNodes().some((node) => node.selected) || getEdges().some((edge) => edge.selected);
                setNodes((nds) => nds.map((node) => ({ ...node, selected: !hasSelection })));
                setEdges((eds) => (eds ?? []).map((edge) => ({ ...edge, selected: false })));
                scheduleSelection();
            }
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [screenToFlowPosition, setNodes, setEdges, getNodes, getEdges, cancelGrab, closeMenu, openGroup, exitGroup, groupSelection, ungroupSelection, editable, canEdit, apply, startGrab, scheduleSelection]);

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

    // fit the view when a loaded project's tab is first shown, or when a group without a saved view is opened.
    // not the fitView prop, that one stays armed on an empty graph and zooms onto the first node added
    const nodesInitialized = useNodesInitialized();
    const fitPendingRef = useRef(fitOnOpen || (!isRoot && !level.graph.viewport));

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

    useEffect(() => {
        // nodes that only got their sizes after the graph was shown (framing on the first draw is done above)
        if (ready && fitPendingRef.current && nodesInitialized && rfInstance && getNodes().length > 0) {
            fitPendingRef.current = false;
            rfInstance.fitView({ maxZoom: 1 });
        }
    }, [ready, nodesInitialized, rfInstance, getNodes]);

    // the backend's graph changed (an op came back, undo, an MCP edit): show it.
    // kept from what's on screen: sizes, nodes mid drag or grab where the cursor has them, and the selection while ops
    // are still on their way (it's ahead of the backend's then)
    const stored = level.graph;
    useEffect(() => {
        if (!state.ready || !stored?.nodes || !stored?.edges || !rfInstance) return;
        const keepSelection = inFlightRef.current > 0;
        const grabbed = new Set(grabRef.current?.nodes.map((n) => n.id) ?? []);
        // functional updates so we merge onto the latest nodes, not a snapshot from before a click/drag that hasn't rendered yet
        setNodes((nds) => {
            const current = new Map((nds ?? []).map((node: any) => [node.id, node]));
            return stored.nodes.map((node: any) => {
                const prev: any = current.get(node.id);
                if (!prev) return node;
                const position = prev.dragging || grabbed.has(node.id) ? prev.position : node.position;
                return { ...node, position, measured: node.measured ?? prev.measured, selected: keepSelection ? prev.selected : !!node.selected, dragging: prev.dragging };
            });
        });
        setEdges((eds) => {
            const current = new Map((eds ?? []).map((edge: any) => [edge.id, edge]));
            return stored.edges.map((edge: any) => {
                const prev: any = current.get(edge.id);
                return { ...edge, selected: keepSelection && prev ? prev.selected : !!edge.selected };
            });
        });
    }, [stored, state.ready, rfInstance]);

    // MARK: -
    // HANDLERS FOR REACT FLOW EVENTS
    const handlePaneClick = useCallback(() => {
        if (grabRef.current) confirmGrab();
    }, [confirmGrab]);

    const handleNodeClickStop = useCallback(
        (event: React.MouseEvent, node: any) => {
            if (grabRef.current) confirmGrab();

            // If shift key is not held, deselect all other nodes
            if (!event.shiftKey) {
                setNodes((nds) => nds.map((n) => ({ ...n, selected: n.id === node.id })));
                scheduleSelection();
            }
        },
        [confirmGrab, setNodes, scheduleSelection]
    );

    const handleNodeDrag = useCallback(() => {
        if (grabRef.current) confirmGrab();
    }, [confirmGrab]);

    // Right Click to cancel
    const handleContextMenu = useCallback(
        (event: MouseEvent | React.MouseEvent<Element, MouseEvent>) => {
            event.preventDefault();
            cancelGrab();
        },
        [cancelGrab]
    );

    useOnViewportChange({
        onStart: () => {
            if (grabRef.current) confirmGrab();
        },
        // keep where the graph is looked at, coming back out of a group restores it. never an undo step
        onEnd: (viewport) => {
            if (editable) ops.apply([{ op: "viewport", viewport }], { commitToHistory: false }).catch(() => {});
        },
    });

    // react flow's sources are inputs and its targets outputs, data flows from the target to the source.
    // connecting to the empty socket on the group input or output adds a group socket, the backend does that
    const onConnect = useCallback(
        (params: Edge | Connection) => {
            if (!editable) return;
            apply([{ op: "connect", from_node: params.target, from_output: params.targetHandle ?? "", to_node: params.source, to_input: params.sourceHandle ?? "" }]);
        },
        [editable, apply]
    );

    // link dragged off a handle and dropped on nothing (the + sign), open the add menu where it was released
    const onConnectEnd = useCallback(
        (event: MouseEvent | TouchEvent, connectionState: FinalConnectionState) => {
            if (connectionState.isValid || connectionState.toHandle || !editable) return;

            const { clientX, clientY } = "changedTouches" in event ? event.changedTouches[0] : event;
            // the new node gets placed from this position, same as shift+a
            mousePositionRef.current = { x: clientX, y: clientY };
            setMenuPosition({ x: clientX, y: clientY });
            setMenuOpen(true);
        },
        [editable]
    );

    // what react flow changes on screen: selection, drags, sizes. removals, drag ends and resizes are sent as ops
    const onNodesChange = useCallback(
        (changes: any[]) => {
            const removed = changes.filter((c) => c.type === "remove").map((c) => c.id);
            setNodes((nds) =>
                applyNodeChanges(
                    changes.filter((c) => c.type !== "remove"),
                    nds
                )
            );
            if (removed.length > 0 && canEdit()) apply([{ op: "delete", nodes: removed }]);
            if (changes.some((c) => c.type === "select")) scheduleSelection();

            // a drag finished
            const moved = changes.filter((c) => c.type === "position" && c.dragging === false);
            if (moved.length > 0 && canEdit()) {
                const current = new Map(getNodes().map((n) => [n.id, n.position]));
                apply([{ op: "move", positions: Object.fromEntries(moved.map((c) => [c.id, c.position ?? current.get(c.id)])) }]);
            }
            // a resize finished
            for (const change of changes.filter((c) => c.type === "dimensions" && c.resizing === false)) {
                const node = getNodes().find((n) => n.id === change.id);
                if (!node || !canEdit()) continue;
                const width = change.dimensions?.width ?? node.width ?? node.measured?.width;
                const height = change.dimensions?.height ?? node.height ?? node.measured?.height;
                if (width && height) apply([{ op: "resize", node: node.id, width, height, position: node.position }]);
            }
        },
        [setNodes, canEdit, apply, scheduleSelection, getNodes]
    );

    const onEdgesChange = useCallback(
        (changes: any[]) => {
            const removed = changes.filter((c) => c.type === "remove").map((c) => c.id);
            setEdges((eds) =>
                applyEdgeChanges(
                    changes.filter((c) => c.type !== "remove"),
                    eds
                )
            );
            if (removed.length > 0 && canEdit()) apply([{ op: "delete", edges: removed }]);
            if (changes.some((c) => c.type === "select")) scheduleSelection();
        },
        [setEdges, canEdit, apply, scheduleSelection]
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
