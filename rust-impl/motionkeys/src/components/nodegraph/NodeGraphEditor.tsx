import { useContext, useEffect, useState, useCallback, useRef, useMemo } from "react";
import { createPortal } from "react-dom";

import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useNodesState, useEdgesState, Connection, Edge, ReactFlowInstance, applyNodeChanges, applyEdgeChanges, useReactFlow, getOutgoers, useOnViewportChange, useStoreApi, useNodesInitialized, useUpdateNodeInternals, FinalConnectionState } from "@xyflow/react";
import { XYHandle } from "@xyflow/system";
import { useStateContext } from "../../contexts/StateContext";
import { NODE_DROP_EVENT } from "../../utils/node";
import { GROUP, GroupDef, Level, NOTE_MAP_NODE, PATH_SEP, Project, inputHandle, outputHandle, specLookup } from "../../utils/groups";
import { ApplyOptions, Op, SocketRef, TabContext, useGraphOps } from "../../utils/graphOps";
import { takeReveal, usePendingReveal } from "../../utils/console";
import { addsToSelection, blockUntilRelease, commandName, isTextField, useHold, useKeymap, useModal } from "../../utils/keymap";
import { LinkFrom, linkSocket, nodeEntries, useNodeSpecs } from "../../utils/nodeEntries";
import { SOCKET_EDIT_EVENT } from "../../nodes/_InterfaceNode";
import NodeGraphCanvas, { CLICK_DISTANCE, useMultiSelection } from "./NodeGraphCanvas";
import NodeAddMenu from "./NodeAddMenu";
import TagMenu from "./TagMenu";
import DeleteUnusedModal from "../DeleteUnusedModal";
import ContextMenu, { ContextEntry } from "./ContextMenu";
import { PICK_UP_EVENT, PickUp, SOCKET_SELECT_EVENT, SocketPoint, SocketSelect, TAG_EDIT_EVENT, TagEdit, draggedAlong, handleSide, sameSocket, sameSockets, selectedSockets, socketPoints } from "./SocketHandle";
import { multiSlotOffset } from "../../utils/sockets";
import { connectAlong } from "./connectAlong";
import { useFrameView } from "./frameView";
import { TOOLBAR_ADD_EVENT, canvasToolCommands, setEditorState } from "../../utils/editorState";
import { Rect, apart, bounds, drawnFrames, dropFrames, frameAncestors, frameAt, isFrame, isLocked, nodeRect, withContents } from "../../utils/frames";
import { ArrangeMode, arrange } from "../../utils/arrange";
import { uncoveredRect } from "../../utils/panels";

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
// add or duplicate was made in, placing ends it and cancelling undoes it. `nodes` are the grabbed ones and what the
// frames among them carry
type Grab = { txn: string | null; cursorX: number; cursorY: number; nodes: { id: string; x: number; y: number }[] };

// a drag or grab under way: what's moved, the frames' rects when it started (for dropping nodes in them, dropFrames), and for a drag where each dragged node started and which dragged frame carries each node in one
type MoveSession = { moved: string[]; frames: Map<string, Rect>; starts: Map<string, { x: number; y: number }>; carriedBy: Map<string, string> };

// a move op, with the frames nodes were dropped in
const moveOp = (positions: Record<string, { x: number; y: number }>, frames: Record<string, string>): Op => ({ op: "move", positions, ...(Object.keys(frames).length > 0 ? { frames } : {}) });

// where the nodes in dragged frames go for these drag changes: as far as the frame they're carried by moved
const carriedChanges = (session: MoveSession, changes: any[]) => {
    const moved = new Map(changes.filter((c) => c.type === "position" && c.position && !session.carriedBy.has(c.id)).map((c) => [c.id, c.position]));
    return [...session.carriedBy]
        .filter(([id, by]) => moved.has(by) && session.starts.has(id) && session.starts.has(by))
        .map(([id, by]) => {
            const [start, from, to] = [session.starts.get(id)!, session.starts.get(by)!, moved.get(by)];
            return { id, type: "position", position: { x: start.x + to.x - from.x, y: start.y + to.y - from.y } };
        });
};

// the right click menu: where it opened (a screen position), what was right clicked, and whether the clipboard had nodes
type ContextMenuState = { at: { x: number; y: number }; target: "nodes" | "edge" | "pane"; canPaste: boolean };

// the node whose height can be dragged too (BaseNode's minHeight)
const VIEWER = "viewer";

// ids of the selected nodes or edges
const selectedIds = (items: { id: string; selected?: boolean }[]) => items.filter((i) => i.selected).map((i) => i.id);

// nodes with exactly these sockets selected, a node keeps which of its sockets are selected
const withSockets = (nodes: any[], sockets: SocketRef[]) =>
    nodes.map(({ selectedSockets, ...node }: any) => {
        const side = (side: "inputs" | "outputs") => sockets.filter((s) => s.node === node.id && s.side === side).map((s) => s.socket);
        const [inputs, outputs] = [side("inputs"), side("outputs")];
        return inputs.length > 0 || outputs.length > 0 ? { ...node, selectedSockets: { inputs, outputs } } : node;
    });

// how long a click on one of several selected sockets waits before selecting it alone, a second click in that time is
// a double click that tags them all
const DOUBLE_CLICK_TIME = 500;

// a link react flow made as an op. react flow's sources are inputs and its targets outputs, data flows from the target
// to the source
const connectOp = (params: Edge | Connection): Op => ({ op: "connect", from_node: params.target, from_output: params.targetHandle ?? "", to_node: params.source, to_input: params.sourceHandle ?? "" });

// edits one graph: selection, adding, grabbing, duplicating, deleting, connecting, grouping. every edit is sent to the
// backend as an op and the graph it sends back is shown, only selection and positions mid drag are ahead of it.
// NodeGraph mounts a fresh one (in its own ReactFlowProvider) for each graph that's opened
function NodeGraphEditor({ level, path, pathGroups, editable, project, openGroup, exitGroup, onReady, shown, fitOnOpen }: EditorProps) {
    // start with the graph already there, the selection is part of it
    const [nodes, setNodes] = useNodesState((level.graph.nodes ?? []).map(({ dragging, measured, resizing, ...node }: any) => node));
    const [edges, setEdges] = useEdgesState(level.graph.edges ?? []);

    const { getNodes, getEdges, screenToFlowPosition } = useReactFlow();
    const store = useStoreApi();
    const frameView = useFrameView();

    const [rfInstance, setRfInstance] = useState(null as ReactFlowInstance | null);

    const [menuOpen, setMenuOpen] = useState(false);
    const [menuPosition, setMenuPosition] = useState({ x: 0, y: 0 });
    // the link the add menu was opened by dragging off a socket, null for shift+a
    const [menuLink, setMenuLink] = useState<LinkFrom | null>(null);
    const mousePositionRef = useRef({ x: 0, y: 0 });

    // the grab in progress, `grabbing` re-renders when one starts or ends
    const grabRef = useRef<Grab | null>(null);
    const [grabbing, setGrabbing] = useState(false);

    // left drag pans while the pan key is held, emulating a 3 button mouse
    const panHeld = useHold("node_editor", "pan");

    const { backEndState: state } = useStateContext();
    const groupId = level.groupId;
    const isRoot = groupId === null;
    const ops = useGraphOps(groupId);

    // read by the window listeners, a graph getting ready behind the one on screen ignores keys and drops
    const shownRef = useRef(shown);
    shownRef.current = shown;

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
    // opened from a link, only the nodes with a socket it can connect to
    const menuEntries = useMemo(() => (menuLink ? entries.filter((entry) => linkSocket(entry, menuLink, nodeSpecs, groups)) : entries), [entries, menuLink, nodeSpecs, groups]);
    const lookup = useMemo(() => specLookup(Object.fromEntries(nodeSpecs.map((spec: any) => [spec.id, spec])), groups), [nodeSpecs, groups]);

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

    // sends ops to the backend, with the sizes the nodes are drawn at for frames to fit around
    const apply = useCallback(
        (list: Op[], options: ApplyOptions = {}) => {
            const sizes = Object.fromEntries(
                getNodes()
                    .filter((n) => !isFrame(n) && n.measured?.width && n.measured?.height)
                    .map((n) => [n.id, { width: n.measured!.width!, height: n.measured!.height! }])
            );
            return track(list.map((o) => o.op).join(", "), () => ops.apply(list, { sizes, ...options }));
        },
        [ops, track, getNodes]
    );

    // MARK: - Selection

    // the selection on screen goes to the backend once the change has rendered (a click selects and deselects in several
    // changes), after a box selection has ended. selecting is an undo step.
    // not on the next frame: react flow's store takes the new selection in an effect after the render, and a frame could
    // come first and send nothing (a click that moved a pixel selected nothing, its move's reply put the old selection back)
    const selectionPendingRef = useRef(false);
    const [selectionChanged, setSelectionChanged] = useState(0);
    const scheduleSelection = useCallback(() => {
        if (selectionPendingRef.current) return;
        selectionPendingRef.current = true;
        inFlightRef.current++;
        setSelectionChanged((n) => n + 1);
    }, []);
    // deselects every socket, for a click that selects something else
    const clearSockets = useCallback(() => {
        if (!getNodes().some((n: any) => n.selectedSockets)) return;
        setNodes((nds) => withSockets(nds, []));
        scheduleSelection();
    }, [getNodes, setNodes, scheduleSelection]);

    // after react flow's own effects (it's a child), so its store has the selection
    useEffect(() => {
        if (!selectionPendingRef.current) return;
        let frame = 0;
        const flush = () => {
            if (store.getState().userSelectionActive) {
                frame = requestAnimationFrame(flush);
                return;
            }
            selectionPendingRef.current = false;
            const nodes = selectedIds(getNodes());
            const edges = selectedIds(getEdges());
            const sockets = selectedSockets(getNodes());
            const stored = storedRef.current;
            const same = (a: string[], b: string[]) => a.length === b.length && a.every((id) => b.includes(id));
            if (canEdit() && !(same(nodes, selectedIds(stored.nodes ?? [])) && same(edges, selectedIds(stored.edges ?? [])) && sameSockets(sockets, selectedSockets(stored.nodes ?? [])))) {
                apply([{ op: "select", nodes, edges, sockets }]);
            }
            inFlightRef.current--;
        };
        flush();
        // a box selection still going is waited on again when this runs again
        return () => cancelAnimationFrame(frame);
    }, [selectionChanged, store, getNodes, getEdges, canEdit, apply]);

    // MARK: - Moving Frames

    // a drag or grab of nodes, it starts with the frames as they're drawn then
    const moveRef = useRef<MoveSession | null>(null);
    const beginMove = useCallback(
        (moved: string[]) => {
            const nodes = getNodes();
            // each node in a dragged frame goes with the innermost dragged frame it's in
            const byId = new Map(nodes.map((n) => [n.id, n]));
            const movedSet = new Set(moved);
            const carriedBy = new Map<string, string>();
            for (const id of withContents(nodes, moved).filter((id) => !movedSet.has(id))) {
                const by = frameAncestors(byId, id).find((f) => movedSet.has(f));
                if (by) carriedBy.set(id, by);
            }
            const starts = new Map(nodes.filter((n) => movedSet.has(n.id) || carriedBy.has(n.id)).map((n) => [n.id, { ...n.position }]));
            moveRef.current = { moved, frames: drawnFrames(nodes), starts, carriedBy };
        },
        [getNodes]
    );
    // the move is over with the nodes at `positions`: the frame each moved node was dropped in, only the ones that change
    const endMove = useCallback(
        (positions: Record<string, { x: number; y: number }>) => {
            const session = moveRef.current;
            moveRef.current = null;
            if (!session) return {};
            const nodes = getNodes().map((n) => (positions[n.id] ? { ...n, position: positions[n.id] } : n));
            return dropFrames(nodes, session.moved, session.frames, screenToFlowPosition(mousePositionRef.current, { snapToGrid: false }));
        },
        [getNodes, screenToFlowPosition]
    );

    // MARK: - Grab

    // the nodes follow the cursor from `cursor` (flow position) until a click places them, a frame takes what's in it along
    const startGrab = useCallback(
        (grabbed: { id: string; position: { x: number; y: number } }[], cursor: { x: number; y: number }, txn: string | null) => {
            const nodes = getNodes();
            const at = new Map([...nodes.map((n) => [n.id, n.position] as const), ...grabbed.map((n) => [n.id, n.position] as const)]);
            const moved = grabbed.map((n) => n.id);
            const ids = [...new Set([...moved, ...withContents(nodes, moved)])].filter((id) => at.has(id));
            grabRef.current = { txn, cursorX: cursor.x, cursorY: cursor.y, nodes: ids.map((id) => ({ id, x: at.get(id)!.x, y: at.get(id)!.y })) };
            beginMove(moved);
            setGrabbing(true);
        },
        [getNodes, beginMove]
    );

    // places the grabbed nodes where they are: one move (into frames they land in), ending the add or duplicate it belongs to
    const confirmGrab = useCallback(() => {
        const grab = grabRef.current;
        grabRef.current = null;
        setGrabbing(false);
        if (!grab) return;
        const placed = new Map(getNodes().map((n) => [n.id, n.position]));
        const positions = Object.fromEntries(grab.nodes.filter((n) => placed.has(n.id)).map((n) => [n.id, placed.get(n.id)!]));
        apply([moveOp(positions, endMove(positions))], grab.txn ? { txn: grab.txn } : {}).then(() => grab.txn && ops.end(grab.txn));
    }, [getNodes, apply, ops, endMove]);

    // puts the grabbed nodes back, an add or duplicate is undone
    const cancelGrab = useCallback(() => {
        const grab = grabRef.current;
        grabRef.current = null;
        setGrabbing(false);
        setMenuOpen(false);
        moveRef.current = null;
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
    // the node (a for each zone brings its other end) is added under the cursor and grabbed until a click places it.
    // opened from a link, the link is connected to the new node's matching socket in the same undo step
    const addEntry = useCallback(
        async (key: string, link: LinkFrom | null) => {
            setMenuOpen(false);
            const entry = entries.find((e) => e.key === key);
            if (!entry || !canEdit()) return;
            const flowPosition = screenToFlowPosition(mousePositionRef.current, { snapToGrid: false });
            const txn = crypto.randomUUID();
            const applied = await apply([{ op: "add_nodes", nodes: [{ type: entry.nodeType, data: entry.data, position: { x: flowPosition.x + 10, y: flowPosition.y + 10 } }] }], { txn });
            if (!applied) return;
            const socket = link && linkSocket(entry, link, nodeSpecs, groups);
            const added = socket && applied.added[socket.node];
            if (link && socket && added) {
                const ends = link.isOutput ? { from_node: link.nodeId, from_output: link.handleId, to_node: added.id, to_input: socket.handle } : { from_node: added.id, from_output: socket.handle, to_node: link.nodeId, to_input: link.handleId };
                await apply([{ op: "connect", ...ends }], { txn });
            }
            startGrab(applied.added, flowPosition, txn);
        },
        [entries, canEdit, screenToFlowPosition, apply, startGrab, nodeSpecs, groups]
    );
    const addNode = useCallback((key: string) => addEntry(key, menuLink), [addEntry, menuLink]);
    // picked from the toolbar's add menu: added under the cursor over the toolbar, the grab brings it down into the graph
    useEffect(() => {
        const handleAdd = (event: Event) => {
            if (shownRef.current) addEntry((event as CustomEvent).detail, null);
        };
        window.addEventListener(TOOLBAR_ADD_EVENT, handleAdd);
        return () => window.removeEventListener(TOOLBAR_ADD_EVENT, handleAdd);
    }, [addEntry]);
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

            // dropped from a floating panel, that window still has the keys. the graph takes them so the next key (X to
            // delete it) goes straight to it
            if (!document.hasFocus()) getCurrentWindow().setFocus();

            // keep the node under the cursor where it was grabbed, in the frame it's dropped on
            const flowPosition = screenToFlowPosition({ x: clientX, y: clientY }, { snapToGrid: false });
            const frame = frameAt(getNodes(), flowPosition);
            apply([{ op: "add_nodes", nodes: [{ type: entry.nodeType, data: entry.data, position: { x: flowPosition.x - offsetX, y: flowPosition.y - offsetY }, ...(frame ? { frame } : {}) }] }]);
        };

        window.addEventListener(NODE_DROP_EVENT, handleDrop);
        return () => window.removeEventListener(NODE_DROP_EVENT, handleDrop);
    }, [store, screenToFlowPosition, entries, canEdit, apply, getNodes]);

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

        window.addEventListener("mousemove", handleMouseMove);
        return () => window.removeEventListener("mousemove", handleMouseMove);
    }, [grabbing, screenToFlowPosition, setNodes]);

    // a grab takes every key and click until it's confirmed or cancelled (left click and right click by default). like
    // blender the confirming click only confirms, react flow never sees it (src/utils/keymap.ts swallows it)
    useModal("grab", { confirm: confirmGrab, cancel: cancelGrab }, grabbing);

    // MARK: - Clipboard

    // copy, cut and paste (the edit menu or its shortcuts) arrive as the page's clipboard events, never as keys (both fire).
    // a text field, or text selected on the page, keeps its own. the graph's go to the backend, which uses the system
    // clipboard. the context menu runs the same ones. each says whether it did anything
    const copy = useCallback(() => {
        const nodes = selectedIds(getNodes());
        if (nodes.length === 0) return false;
        ops.copy(nodes).catch((e) => console.error(`copy: ${e}`));
        return true;
    }, [getNodes, ops]);

    const cut = useCallback(() => {
        const nodes = selectedIds(getNodes());
        const edges = selectedIds(getEdges());
        const sockets = selectedSockets(getNodes());
        if (!canEdit() || (nodes.length === 0 && edges.length === 0 && sockets.length === 0)) return false;
        track("cut", () => ops.cut(nodes, edges, sockets));
        return true;
    }, [getNodes, getEdges, canEdit, ops, track]);

    // centered on `at` (a screen position). pasting again in the same place steps each paste down and to the right
    const lastPasteRef = useRef<{ x: number; y: number; count: number } | null>(null);
    const paste = useCallback(
        (at: { x: number; y: number }) => {
            if (!canEdit()) return false;
            const position = screenToFlowPosition(at, { snapToGrid: false });
            const last = lastPasteRef.current;
            const count = last && Math.abs(last.x - position.x) < 1 && Math.abs(last.y - position.y) < 1 ? last.count + 1 : 0;
            lastPasteRef.current = { ...position, count };
            track("paste", () => ops.paste({ x: position.x + 20 * count, y: position.y + 20 * count }));
            return true;
        },
        [canEdit, screenToFlowPosition, ops, track]
    );

    useEffect(() => {
        const ownsClipboard = () => shownRef.current && !isTextField(document.activeElement) && !window.getSelection()?.toString();

        const handleCopy = (event: ClipboardEvent) => {
            if (ownsClipboard() && copy()) event.preventDefault();
        };

        const handleCut = (event: ClipboardEvent) => {
            if (ownsClipboard() && cut()) event.preventDefault();
        };

        // under the cursor when it's over the graph, otherwise in the middle of the view
        const handlePaste = (event: ClipboardEvent) => {
            const rect = store.getState().domNode?.getBoundingClientRect();
            if (!ownsClipboard() || !rect) return;
            const { x, y } = mousePositionRef.current;
            const over = x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
            if (paste(over ? { x, y } : { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 })) event.preventDefault();
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
    }, [getNodes, copy, cut, paste, store]);

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

    // Shift+P: the selected nodes go in a new frame, which becomes the selection
    const joinInFrame = useCallback(() => {
        const nodes = selectedIds(getNodes());
        if (nodes.length > 0) apply([{ op: "join_in_frame", nodes }]);
    }, [getNodes, apply]);

    // Alt+P: the selected nodes in a frame go out of it, the ones in a frame that's selected too stay in it
    const removeFromFrame = useCallback(() => {
        const nodes = getNodes();
        const byId = new Map(nodes.map((n) => [n.id, n]));
        const selected = new Set(selectedIds(nodes));
        const inFrames = nodes.filter((n: any) => n.selected && n.frame && !frameAncestors(byId, n.id).some((f) => selected.has(f))).map((n) => n.id);
        if (inFrames.length > 0) apply([{ op: "remove_from_frame", nodes: inFrames }]);
    }, [getNodes, apply]);

    // collapses nodes to their headers or expands them, only the ones that change. frames stay as they are
    const collapse = useCallback(
        (nodes: any[], collapsed: boolean) => {
            const changed = nodes.filter((n) => !isFrame(n) && !!n.data?.collapsed !== collapsed).map((n) => n.id);
            if (changed.length > 0) apply([{ op: "collapse", nodes: changed, collapsed }]);
        },
        [apply]
    );
    // collapse all and expand all act on the selection, or on every node when nothing is selected
    const collapseTargets = useCallback(() => {
        const selected = getNodes().filter((n) => n.selected);
        return selected.length > 0 ? selected : getNodes();
    }, [getNodes]);

    // the selected nodes (every node when none are) laid out left to right from their top left, in one move, placed in
    // their columns the way settings say. the view stays unless they all end up out of it, then they're framed once
    // they're drawn there
    const arrangeNodes = useCallback(async () => {
        const shownNodes = getNodes();
        const measured = new Map(shownNodes.map((n) => [n.id, n.measured]));
        const nodes = (storedRef.current.nodes ?? []).map((n: any) => ({ ...n, measured: measured.get(n.id) }));
        const selected = selectedIds(shownNodes);
        const sockets = Object.fromEntries([...store.getState().nodeLookup.values()].map((node) => [node.id, socketPoints(node).map((p) => ({ side: p.side, socket: p.socket, y: p.y - node.internals.positionAbsolute.y }))]));
        const mode = await invoke<any>("get_settings").then(
            (settings) => settings?.node_editor?.arrange as ArrangeMode,
            () => undefined
        );
        const positions = await arrange(nodes, getEdges(), selected, sockets, mode).catch((e): Record<string, { x: number; y: number }> => {
            console.error(`arrange: ${e}`);
            return {};
        });
        if (Object.keys(positions).length === 0 || !(await apply([{ op: "move", positions }]))) return;

        const { domNode } = store.getState();
        if (!domNode) return;
        const visible = uncoveredRect(domNode);
        const topLeft = screenToFlowPosition({ x: visible.left, y: visible.top }, { snapToGrid: false });
        const bottomRight = screenToFlowPosition({ x: visible.left + visible.width, y: visible.top + visible.height }, { snapToGrid: false });
        const box = bounds(nodes.filter((n) => positions[n.id]).map((n) => ({ ...nodeRect(n), ...positions[n.id] })));
        if (!apart(box, { ...topLeft, width: bottomRight.x - topLeft.x, height: bottomRight.y - topLeft.y })) return;
        requestAnimationFrame(() => requestAnimationFrame(() => frameView(selected.length > 0 ? getNodes().filter((n) => n.selected) : undefined)));
    }, [getNodes, getEdges, store, apply, screenToFlowPosition, frameView]);

    // the nodes Delete Unused Nodes found, asked about before they go. frames emptied by it stay
    const [unused, setUnused] = useState<string[] | null>(null);
    const deleteUnused = useCallback(() => {
        if (!unused) return;
        setUnused(null);
        apply([{ op: "delete", nodes: unused, keep_frames: true }]);
    }, [unused, apply]);
    const cancelUnused = useCallback(() => setUnused(null), []);

    // what the toolbar's buttons can do here, said by the graph on screen
    const selectedNodes = nodes.filter((n) => n.selected);
    const selectedCount = selectedNodes.length;
    const groupCount = selectedNodes.filter((n) => n.type === GROUP).length;
    const openableCount = selectedNodes.filter((n) => n.type === GROUP || n.type === NOTE_MAP_NODE).length;
    useEffect(() => {
        if (shown) setEditorState({ editable, selected: selectedCount, groups: groupCount, openable: openableCount, inGroup: !isRoot, entries });
    }, [shown, editable, selectedCount, groupCount, openableCount, isRoot, entries]);

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

    // MARK: - Sockets and tags

    // a socket clicked: it alone is selected, or it's added to (or taken out of) the selection with shift or cmd
    const selectSocket = useCallback(
        (clicked: SocketRef, add: boolean) => {
            const current = selectedSockets(getNodes());
            const next = !add ? [clicked] : current.some((s) => sameSocket(s, clicked)) ? current.filter((s) => !sameSocket(s, clicked)) : [...current, clicked];
            setNodes((nds) => withSockets(nds, next).map((n) => ({ ...n, selected: add ? n.selected : false })));
            if (!add) setEdges((eds) => (eds ?? []).map((edge) => ({ ...edge, selected: false })));
            scheduleSelection();
        },
        [getNodes, setNodes, setEdges, scheduleSelection]
    );
    // a click on one of several selected sockets waiting to select it alone, a double click keeps them all
    const collapseRef = useRef<number | null>(null);
    const cancelCollapse = useCallback(() => {
        if (collapseRef.current !== null) clearTimeout(collapseRef.current);
        collapseRef.current = null;
    }, []);
    useEffect(() => cancelCollapse, [cancelCollapse]);
    useEffect(() => {
        const handleSocketSelect = (event: Event) => {
            const { scope, node, side, socket, add } = (event as CustomEvent).detail as SocketSelect;
            if (!shownRef.current || scope !== groupId) return;
            cancelCollapse();
            const clicked = { node, side, socket };
            const current = selectedSockets(getNodes());
            if (!add && current.length > 1 && current.some((s) => sameSocket(s, clicked))) {
                collapseRef.current = window.setTimeout(() => {
                    collapseRef.current = null;
                    selectSocket(clicked, false);
                }, DOUBLE_CLICK_TIME);
                return;
            }
            selectSocket(clicked, add);
        };
        window.addEventListener(SOCKET_SELECT_EVENT, handleSocketSelect);
        return () => window.removeEventListener(SOCKET_SELECT_EVENT, handleSocketSelect);
    }, [groupId, getNodes, selectSocket, cancelCollapse]);

    // the socket whose tag menu is open, and every socket the menu tags: the selected ones on its side when it's one of
    // them, top to bottom
    const [tagEdit, setTagEdit] = useState<(TagEdit & { sockets: SocketRef[] }) | null>(null);
    useEffect(() => {
        const handleTagEdit = (event: Event) => {
            const detail = (event as CustomEvent).detail as TagEdit;
            if (!shownRef.current || detail.scope !== groupId || !canEdit()) return;
            cancelCollapse();
            const edited = { node: detail.node, side: detail.side, socket: detail.socket };
            const selected = selectedSockets(getNodes()).filter((s) => s.side === edited.side);
            let sockets: SocketRef[] = [edited];
            if (selected.length > 1 && selected.some((s) => sameSocket(s, edited))) {
                const points = [...store.getState().nodeLookup.values()].flatMap(socketPoints);
                const at = (s: SocketRef) => points.find((p) => sameSocket(p, s)) ?? { x: 0, y: 0 };
                sockets = [...selected].sort((a, b) => at(a).y - at(b).y || at(a).x - at(b).x);
            }
            setTagEdit({ ...detail, sockets });
        };
        window.addEventListener(TAG_EDIT_EVENT, handleTagEdit);
        return () => window.removeEventListener(TAG_EDIT_EVENT, handleTagEdit);
    }, [groupId, canEdit, cancelCollapse, getNodes, store]);
    const closeTagMenu = useCallback(() => setTagEdit(null), []);

    // what the menu offers: an input the tags of the outputs here, an output the broken tags on inputs
    const tagNames = useMemo(() => {
        if (!tagEdit) return [];
        const graphNodes: any[] = level.graph.nodes ?? [];
        const outputs = new Set(graphNodes.flatMap((n) => Object.values(n.data?.output_tags ?? {}) as string[]));
        if (tagEdit.side === "inputs") return [...outputs].sort();
        const broken = new Set(graphNodes.flatMap((n) => Object.values(n.data?.input_tags ?? {}) as string[]).filter((name) => !outputs.has(name)));
        return [...broken].sort();
    }, [tagEdit, level.graph.nodes]);
    const tagNode = tagEdit && (level.graph.nodes ?? []).find((n: any) => n.id === tagEdit.node);
    const currentTag: string = (tagEdit && tagNode?.data?.[tagEdit.side === "inputs" ? "input_tags" : "output_tags"]?.[tagEdit.socket]) ?? "";

    const setTag = useCallback(
        (name: string) => {
            if (!tagEdit) return;
            setTagEdit(null);
            if (tagEdit.sockets.length > 1) {
                apply([{ op: "set_tags", sockets: tagEdit.sockets, name }]);
                return;
            }
            if (name === currentTag) return;
            apply([{ op: "set_tag", node: tagEdit.node, side: tagEdit.side, socket: tagEdit.socket, name }]);
        },
        [tagEdit, currentTag, apply]
    );

    // MARK: - Commands

    // the add menu at `at` (a screen position), shift+a opens it under the cursor
    const openAddMenu = useCallback(
        (at: { x: number; y: number }) => {
            if (!canEdit()) return;
            setMenuPosition(at);
            setMenuLink(null);
            setMenuOpen(true);
        },
        [canEdit]
    );

    // the node editor's commands, their keys come from the keymap (src/utils/keymap.ts). a graph getting ready behind the
    // one on screen doesn't take them. the context menu runs them by id
    const runEditorCommand = useKeymap(
        "node_editor",
        {
            // tab opens the selected group (or a note map), or goes back out when no group is selected
            edit_group: () => {
                const selectedGroups = getNodes().filter((n) => n.selected && (n.type === GROUP || n.type === NOTE_MAP_NODE));
                if (selectedGroups.length === 1) openGroup(selectedGroups[0].id);
                else exitGroup(false);
            },
            exit_group: () => exitGroup(false),
            exit_to_root: () => exitGroup(true),
            frame_all: () => frameView(),
            // frames everything when nothing is selected
            frame_selected: () => frameView(getNodes().filter((n) => n.selected)),
            ...canvasToolCommands,
            group: () => {
                if (canEdit()) groupSelection();
            },
            ungroup: () => {
                if (canEdit()) ungroupSelection();
            },
            join_in_frame: () => {
                if (canEdit()) joinInFrame();
            },
            remove_from_frame: () => {
                if (canEdit()) removeFromFrame();
            },
            // H like blender: the selected nodes collapse, or expand when they all are
            collapse: () => {
                const selected = getNodes().filter((n) => n.selected && !isFrame(n));
                if (canEdit()) collapse(selected, !selected.every((n: any) => n.data?.collapsed));
            },
            arrange: () => {
                if (canEdit()) arrangeNodes();
            },
            collapse_all: () => {
                if (canEdit()) collapse(collapseTargets(), true);
            },
            expand_all: () => {
                if (canEdit()) collapse(collapseTargets(), false);
            },
            add_node: () => openAddMenu({ ...mousePositionRef.current }),
            // the selected nodes (a zone as a pair) and edges, edges of deleted nodes go with them.
            // read from the store, the closure's nodes can be a render behind
            delete: () => {
                if (!canEdit()) return;
                const nodes = selectedIds(getNodes());
                const edges = selectedIds(getEdges());
                const sockets = selectedSockets(getNodes());
                if (nodes.length === 0 && edges.length === 0 && sockets.length === 0) return;
                apply([{ op: "delete", nodes, edges, sockets }]);
            },
            // nothing unused, nothing asked
            delete_unused: async () => {
                if (!canEdit()) return;
                const nodes = await ops.unused().catch((e) => {
                    console.error(`graph_unused: ${e}`);
                    return [];
                });
                if (nodes.length > 0 && shownRef.current) setUnused(nodes);
            },
            // a locked frame stays where it is
            grab: () => {
                if (!editable) return;
                const selected = getNodes().filter((node) => node.selected && !isLocked(node));
                if (selected.length === 0) return;
                const { x, y } = mousePositionRef.current;
                startGrab(selected, screenToFlowPosition({ x, y }, { snapToGrid: false }), null);
            },
            // the copies are grabbed, placing them ends the duplicate and cancelling undoes it
            duplicate: async () => {
                if (!canEdit()) return;
                const selected = selectedIds(getNodes());
                if (selected.length === 0) return;
                const { x, y } = mousePositionRef.current;
                const flowPosition = screenToFlowPosition({ x, y }, { snapToGrid: false });
                const txn = crypto.randomUUID();
                const applied = await apply([{ op: "duplicate", nodes: selected, offset: { x: 20, y: 20 } }], { txn });
                if (applied) startGrab(applied.added, flowPosition, txn);
            },
            // deselect everything if anything is selected, otherwise select every node but the locked frames
            select_all: () => {
                const hasSelection = getNodes().some((node: any) => node.selected || node.selectedSockets) || getEdges().some((edge) => edge.selected);
                setNodes((nds) => withSockets(nds, []).map((node) => ({ ...node, selected: !hasSelection && !isLocked(node) })));
                setEdges((eds) => (eds ?? []).map((edge) => ({ ...edge, selected: false })));
                scheduleSelection();
            },
        },
        () => shownRef.current
    );

    // react flow's own drags (box select, dragging a link or a node) take every key until the mouse is let go, so a key
    // can't open a group or start a grab halfway through one. a node drag is a move: frames take what's in them along,
    // nodes go in the frames they land in
    const onNodeDragStart = useCallback(
        (_event: unknown, _node: unknown, dragged: { id: string }[]) => {
            blockUntilRelease();
            beginMove(dragged.map((n) => n.id));
        },
        [beginMove]
    );
    const onSelectionDragStart = useCallback((event: unknown, dragged: { id: string }[]) => onNodeDragStart(event, null, dragged), [onNodeDragStart]);
    // a link drag cancelled partway through (a right click) drops nowhere: nothing connects and a picked up link goes
    // back. react flow's drag ends at the next mouse move or release, the handlers below do nothing then
    const linkCancelledRef = useRef(false);
    const blockLink = useCallback(() => {
        linkCancelledRef.current = false;
        blockUntilRelease("link_drag", {
            cancel: () => {
                linkCancelledRef.current = true;
                store.getState().cancelConnection();
            },
        });
    }, [store]);
    // where a link drag started, letting go without moving is a click on the socket (the first of a double click to tag
    // it), not a link dropped on nothing
    const linkStartRef = useRef<{ x: number; y: number } | null>(null);
    // the other selected sockets a link dragged off a selected socket brings along, and that link once it's dropped on a
    // socket (they're connected together when the drag ends)
    const alongRef = useRef<SocketPoint[]>([]);
    const droppedRef = useRef<Edge | Connection | null>(null);
    const onConnectStart = useCallback(
        (event: MouseEvent | TouchEvent, { nodeId, handleId, handleType }: { nodeId: string | null; handleId: string | null; handleType: "source" | "target" | null }) => {
            const { clientX, clientY } = "touches" in event ? event.touches[0] : event;
            linkStartRef.current = { x: clientX, y: clientY };
            alongRef.current = nodeId && handleId && handleType ? draggedAlong(store.getState().nodeLookup, { node: nodeId, side: handleSide(handleType), socket: handleId }) : [];
            droppedRef.current = null;
            blockLink();
        },
        [blockLink, store]
    );

    // MARK: - Picking Up Links

    // a press on a connected input picks up its link like blender: the link's input end follows the mouse from its output.
    // dropped on an input it moves there, on nothing it's removed, a click leaves it. a multi input gives up the link whose
    // slot is nearest the press. one of several selected sockets starts a new link like an empty one, the others come along
    const pickedRef = useRef<string | null>(null);
    useEffect(() => {
        const handlePickUp = (event: Event) => {
            const { scope, node, socket, mouse } = (event as CustomEvent).detail as PickUp;
            if (!shownRef.current || scope !== groupId) return;
            const state = store.getState();
            // what react flow's own socket drag is given
            const drag = {
                autoPanOnConnect: state.autoPanOnConnect,
                autoPanSpeed: state.autoPanSpeed,
                connectionMode: state.connectionMode,
                connectionRadius: state.connectionRadius,
                domNode: state.domNode,
                nodeLookup: state.nodeLookup,
                lib: state.lib,
                flowId: state.rfId,
                panBy: state.panBy,
                cancelConnection: state.cancelConnection,
                updateConnection: state.updateConnection,
                isValidConnection: state.isValidConnection,
                getTransform: () => store.getState().transform,
                getFromHandle: () => store.getState().connection.fromHandle,
            };

            // the links into the input, without the connection a tag makes
            const input = { node, side: "inputs" as const, socket };
            const into = getEdges().filter((e: any) => e.source === node && e.sourceHandle === socket && !e.tagged);
            if (into.length === 0 || draggedAlong(state.nodeLookup, input).length > 0) {
                XYHandle.onPointerDown(mouse, { ...drag, nodeId: node, handleId: socket, isTarget: false, onConnectStart: state.onConnectStart, onConnectEnd: state.onConnectEnd, onConnect: (connection) => state.onConnect?.(connection) });
                return;
            }

            // the link whose slot is nearest the press, the links into a multi input are spread over its socket in order
            const center = socketPoints(state.nodeLookup.get(node)!).find((p) => sameSocket(p, input))?.y ?? 0;
            const pressY = screenToFlowPosition({ x: mouse.clientX, y: mouse.clientY }, { snapToGrid: false }).y;
            const distance = (index: number) => Math.abs(center + multiSlotOffset(index, into.length) - pressY);
            const picked = into[into.map((_, i) => i).reduce((best, i) => (distance(i) < distance(best) ? i : best), 0)];

            // hidden once the mouse moves far enough to be a drag, not a click
            let moved = false;
            const follow = (move: MouseEvent) => {
                if (moved || Math.hypot(move.clientX - mouse.clientX, move.clientY - mouse.clientY) <= CLICK_DISTANCE) return;
                moved = true;
                pickedRef.current = picked.id;
                setEdges((eds) => eds.map((e) => (e.id === picked.id ? { ...e, hidden: true } : e)));
            };
            const restore = () => {
                pickedRef.current = null;
                setEdges((eds) => eds.map((e) => (e.id === picked.id ? { ...e, hidden: false } : e)));
            };
            window.addEventListener("mousemove", follow);

            // dragged from the output end, react flow's reconnect
            let dropped: Connection | null = null;
            blockLink();
            XYHandle.onPointerDown(mouse, {
                ...drag,
                nodeId: picked.target,
                handleId: picked.targetHandle ?? null,
                isTarget: true,
                edgeUpdaterType: "target",
                onConnect: (connection) => (dropped = connection),
                onReconnectEnd: () => {
                    window.removeEventListener("mousemove", follow);
                    if (!moved) return;
                    // back where it was or cancelled, nothing changes
                    const same = dropped && dropped.source === picked.source && dropped.sourceHandle === picked.sourceHandle && dropped.target === picked.target && dropped.targetHandle === picked.targetHandle;
                    if (same || !canEdit() || linkCancelledRef.current) {
                        restore();
                        return;
                    }
                    // one undo step for taking it off and putting it on another input
                    apply([{ op: "delete", edges: [picked.id] }, ...(dropped ? [connectOp(dropped)] : [])]).finally(() => (pickedRef.current = null));
                },
            });
        };
        window.addEventListener(PICK_UP_EVENT, handlePickUp);
        return () => window.removeEventListener(PICK_UP_EVENT, handlePickUp);
    }, [groupId, store, getEdges, setEdges, screenToFlowPosition, blockLink, canEdit, apply]);

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

    useMultiSelection();

    // react flow measures where a node's sockets are when the node is drawn or changes size. a measurement can land on a
    // node that an update from the backend has just replaced (it happens most on the first frames after a restart),
    // leaving the node its size but no sockets, and it's never measured again while its size stays the same. edges to it
    // aren't drawn and no link can be dragged off it. a node like that, or with other sockets on screen than measured, is
    // measured again
    const updateNodeInternals = useUpdateNodeInternals();
    useEffect(() => {
        const { nodeLookup, domNode } = store.getState();
        const stale = [...nodeLookup.values()]
            .filter((node) => {
                const element = node.measured?.width ? domNode?.querySelector(`.react-flow__node[data-id="${CSS.escape(node.id)}"]`) : null;
                if (!element) return false;
                const bounds = node.internals.handleBounds;
                // the same sockets react flow measures
                return !bounds || element.querySelectorAll(".source").length !== (bounds.source?.length ?? 0) || element.querySelectorAll(".target").length !== (bounds.target?.length ?? 0);
            })
            .map((node) => node.id);
        if (stale.length > 0) updateNodeInternals(stale);
    });

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
                frameView();
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
            const empty = store.getState().domNode?.querySelectorAll(".react-flow__node .node-header > .node-label:empty").length ?? 0;
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
    }, [ready, rfInstance, getNodes, store, onReady, frameView]);

    // a graph shown empty has nothing to frame, the first node added stays where it was put
    const shownEmpty = ready && state.ready && level.graph.nodes?.length === 0;
    useEffect(() => {
        if (shownEmpty) fitPendingRef.current = false;
    }, [shownEmpty]);

    useEffect(() => {
        // nodes that only got their sizes after the graph was shown (framing on the first draw is done above)
        if (ready && fitPendingRef.current && nodesInitialized && rfInstance && getNodes().length > 0) {
            fitPendingRef.current = false;
            frameView();
        }
    }, [ready, nodesInitialized, rfInstance, getNodes, frameView]);

    // a node in this graph the console asked for (src/utils/console.ts): selected on its own and framed like frame
    // selected, once this graph is on screen
    const reveal = usePendingReveal();
    const tab = useContext(TabContext);
    useEffect(() => {
        if (!reveal || reveal.tab !== tab || !ready || !shown) return;
        const parts = reveal.node.split(PATH_SEP);
        if (parts.slice(0, -1).join(PATH_SEP) !== path.join(PATH_SEP)) return;
        takeReveal(reveal.seq);
        const id = parts[parts.length - 1];
        const node = getNodes().find((n) => n.id === id);
        if (!node) return;
        setNodes((nds) => withSockets(nds, []).map((n) => ({ ...n, selected: n.id === id })));
        setEdges((eds) => (eds ?? []).map((edge) => ({ ...edge, selected: false })));
        scheduleSelection();
        frameView([node]);
    }, [reveal, tab, ready, shown, path, getNodes, setNodes, setEdges, scheduleSelection, frameView]);

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
            // sizes only ever come from what react flow measured on screen. the backend's are whatever was measured when
            // it was last told (a saved project keeps them), and react flow doesn't measure a node again until its size
            // changes, so a stale one would stick (framing and edges going by the wrong size)
            return stored.nodes.map(({ measured, ...node }: any) => {
                const prev: any = current.get(node.id);
                if (!prev) return node;
                const position = prev.dragging || grabbed.has(node.id) ? prev.position : node.position;
                const { selectedSockets, ...stored } = node;
                const sockets = keepSelection ? prev.selectedSockets : selectedSockets;
                return { ...stored, position, measured: prev.measured, selected: keepSelection ? prev.selected : !!node.selected, dragging: prev.dragging, ...(sockets ? { selectedSockets: sockets } : {}) };
            });
        });
        setEdges((eds) => {
            const current = new Map((eds ?? []).map((edge: any) => [edge.id, edge]));
            return stored.edges.map((edge: any) => {
                const prev: any = current.get(edge.id);
                // a link being picked up stays hidden while it's dragged
                const picked = edge.id === pickedRef.current ? { hidden: true } : {};
                return { ...edge, selected: keepSelection && prev ? prev.selected : !!edge.selected, ...picked };
            });
        });
    }, [stored, state.ready, rfInstance]);

    // MARK: -
    // HANDLERS FOR REACT FLOW EVENTS
    // a click while grabbing never reaches react flow, the grab modal confirms or cancels it
    // a double click on a group (or a note map) opens it, not one on its sockets or fields
    const handleNodeDoubleClick = useCallback(
        (event: React.MouseEvent, node: any) => {
            if ((event.target as Element).closest(".nodrag, .react-flow__handle")) return;
            if (node.type === GROUP || node.type === NOTE_MAP_NODE) openGroup(node.id);
        },
        [openGroup]
    );

    const handleNodeClickStop = useCallback(
        (event: React.MouseEvent, node: any) => {
            // If shift (or cmd) is not held, deselect all other nodes
            if (!addsToSelection(event)) {
                setNodes((nds) => withSockets(nds, []).map((n) => ({ ...n, selected: n.id === node.id })));
                scheduleSelection();
            } else if (isFrame(node)) {
                // react flow doesn't select frames, shift or cmd adds or takes one out here
                setNodes((nds) => nds.map((n) => (n.id === node.id ? { ...n, selected: !n.selected } : n)));
                scheduleSelection();
            }
        },
        [setNodes, scheduleSelection]
    );

    // clicking an edge or the empty graph, or starting a box selection, without shift or cmd leaves no socket selected.
    // react flow only deselects the nodes and edges it knows are selected
    const handleEdgeClick = useCallback((event: React.MouseEvent) => !addsToSelection(event) && clearSockets(), [clearSockets]);
    // the innermost frame under the mouse whose empty inside a click selects, a locked one's inside is the graph's
    const frameUnder = useCallback(
        (event: React.MouseEvent | MouseEvent) => {
            const nodes = getNodes();
            const id = frameAt(nodes, screenToFlowPosition({ x: event.clientX, y: event.clientY }, { snapToGrid: false }), true);
            return nodes.find((n) => n.id === id) ?? null;
        },
        [getNodes, screenToFlowPosition]
    );
    // a click on a frame's empty inside selects the frame, shift or cmd adds it or takes it out and keeps what was selected
    // before the press (react flow deselects everything on it). react flow deselects everything again right after the
    // click, `keepRef` holds what stays selected until then
    const keepRef = useRef<Set<string> | null>(null);
    const handlePaneClick = useCallback(
        (event: React.MouseEvent) => {
            clickedRef.current = true;
            clearSockets();
            const frame = frameUnder(event);
            if (!frame) return;
            const before = boxBeforeRef.current;
            const add = addsToSelection(event);
            const keep = new Set(add ? [...before.nodes, ...before.edges] : []);
            if (add && before.nodes.includes(frame.id)) keep.delete(frame.id);
            else keep.add(frame.id);
            keepRef.current = keep;
            queueMicrotask(() => (keepRef.current = null));
            setNodes((nds) => nds.map((n) => ({ ...n, selected: keep.has(n.id) })));
            setEdges((eds) => (eds ?? []).map((e) => ({ ...e, selected: keep.has(e.id) })));
            scheduleSelection();
        },
        [clearSockets, frameUnder, setNodes, setEdges, scheduleSelection]
    );
    // react flow only takes a press on the graph as a click when the mouse didn't move at all, one that moved a pixel was
    // an empty box select. one that ends within the click distance (blender's 3px) is a click too
    const clickedRef = useRef(false);
    const pressRef = useRef<{ x: number; y: number } | null>(null);
    const onSelectionEnd = useCallback(
        (event: React.MouseEvent) => {
            const press = pressRef.current;
            pressRef.current = null;
            if (!clickedRef.current && press && Math.hypot(event.clientX - press.x, event.clientY - press.y) <= CLICK_DISTANCE) handlePaneClick(event);
        },
        [handlePaneClick]
    );
    // react flow's changes without deselecting what a click on a frame keeps
    const keptChanges = (changes: any[]) => {
        const keep = keepRef.current;
        return keep ? changes.filter((c) => !(c.type === "select" && !c.selected && keep.has(c.id))) : changes;
    };
    // the sockets selected before a box selection with shift or cmd, the sockets in the box are added to them
    const boxBaseRef = useRef<SocketRef[]>([]);
    // everything selected before a box selection, cancelling it (a right click partway through) puts that back. react
    // flow's box goes and the moves until the mouse is let go select nothing
    const boxBeforeRef = useRef<{ nodes: string[]; edges: string[]; sockets: SocketRef[] }>({ nodes: [], edges: [], sockets: [] });
    const cancelBox = useCallback(() => {
        if (!store.getState().userSelectionRect) return;
        const before = boxBeforeRef.current;
        store.setState({ userSelectionActive: false, userSelectionRect: null });
        setNodes((nds) => withSockets(nds, before.sockets).map((node) => ({ ...node, selected: before.nodes.includes(node.id) })));
        setEdges((eds) => (eds ?? []).map((edge) => ({ ...edge, selected: before.edges.includes(edge.id) })));
        scheduleSelection();
    }, [store, setNodes, setEdges, scheduleSelection]);
    const onSelectionStart = useCallback(
        (event: React.MouseEvent) => {
            clickedRef.current = false;
            pressRef.current = { x: event.clientX, y: event.clientY };
            const add = addsToSelection(event);
            boxBaseRef.current = add ? selectedSockets(getNodes()) : [];
            boxBeforeRef.current = { nodes: selectedIds(getNodes()), edges: selectedIds(getEdges()), sockets: selectedSockets(getNodes()) };
            if (!add) clearSockets();
            blockUntilRelease("box_select", { cancel: cancelBox });
        },
        [getNodes, getEdges, clearSockets, cancelBox]
    );
    // the sockets inside a box selection (NodeGraphCanvas's BoxSelect), while it's dragged
    const onBoxSockets = useCallback(
        (sockets: SocketRef[]) => {
            const base = boxBaseRef.current;
            const next = [...base, ...sockets.filter((s) => !base.some((b) => sameSocket(b, s)))];
            if (sameSockets(next, selectedSockets(getNodes()))) return;
            setNodes((nds) => withSockets(nds, next));
            scheduleSelection();
        },
        [getNodes, setNodes, scheduleSelection]
    );

    // react flow's drag keeps mouse moves from the window, the cursor is taken from the drag (where it drops, dropFrames)
    const handleNodeDrag = useCallback(
        (event: React.MouseEvent | MouseEvent) => {
            mousePositionRef.current = { x: event.clientX, y: event.clientY };
            if (grabRef.current) confirmGrab();
        },
        [confirmGrab]
    );

    // no browser context menu on the graph
    const handleContextMenu = useCallback((event: MouseEvent | React.MouseEvent<Element, MouseEvent>) => {
        event.preventDefault();
    }, []);

    // MARK: - Context Menu

    // right click opens the menu for what's under the cursor: nodes, an edge or the empty graph. a right click during a
    // modal cancels it instead and never gets here (src/utils/keymap.ts), one on a text field opens nothing
    const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
    const closeContextMenu = useCallback(() => setContextMenu(null), []);
    // the latest right click, a slower one's answer about the clipboard doesn't open its menu
    const contextRequestRef = useRef(0);
    const openContextMenu = useCallback(
        (event: MouseEvent | React.MouseEvent, target: ContextMenuState["target"]) => {
            const at = { x: event.clientX, y: event.clientY };
            const request = ++contextRequestRef.current;
            const show = (canPaste: boolean) => {
                if (request === contextRequestRef.current) setContextMenu({ at, target, canPaste });
            };
            // paste is only there when the clipboard has nodes, asked before the menu shows
            if (target !== "edge" && editable) invoke<boolean>("graph_can_paste").then(show, () => show(false));
            else show(false);
        },
        [editable]
    );
    // a node or edge right clicked outside the selection becomes the selection, like a click on it
    const selectOnly = useCallback(
        (nodeId: string | null, edgeId: string | null) => {
            setNodes((nds) => withSockets(nds, []).map((node) => ({ ...node, selected: node.id === nodeId })));
            setEdges((eds) => (eds ?? []).map((edge) => ({ ...edge, selected: edge.id === edgeId })));
            scheduleSelection();
        },
        [setNodes, setEdges, scheduleSelection]
    );
    const handleNodeContextMenu = useCallback(
        (event: React.MouseEvent, node: any) => {
            event.preventDefault();
            if (isTextField(event.target as Element)) return;
            if (!node.selected) selectOnly(node.id, null);
            openContextMenu(event, "nodes");
        },
        [selectOnly, openContextMenu]
    );
    const handleSelectionContextMenu = useCallback(
        (event: React.MouseEvent) => {
            event.preventDefault();
            openContextMenu(event, "nodes");
        },
        [openContextMenu]
    );
    // delete is all an edge has, a graph that can't be edited has nothing to show
    const handleEdgeContextMenu = useCallback(
        (event: React.MouseEvent, edge: Edge) => {
            event.preventDefault();
            if (!editable) return;
            if (!edge.selected) selectOnly(null, edge.id);
            openContextMenu(event, "edge");
        },
        [editable, selectOnly, openContextMenu]
    );
    // inside a frame it's the frame's menu
    const handlePaneContextMenu = useCallback(
        (event: MouseEvent | React.MouseEvent) => {
            event.preventDefault();
            const frame = frameUnder(event);
            if (frame && !frame.selected) selectOnly(frame.id, null);
            openContextMenu(event, frame ? "nodes" : "pane");
        },
        [frameUnder, selectOnly, openContextMenu]
    );
    // another tab or group shown closes it, and the delete unused modal
    useEffect(() => {
        if (shown) return;
        setContextMenu(null);
        setUnused(null);
    }, [shown]);

    // the entries for what was right clicked, named and run like the same commands in the menus and the toolbar. one that
    // can't run here isn't there. paste and add node go where the menu was opened. frames are a section of their own
    // after delete, remove from frame only with a node in one selected
    const contextSections = (menu: ContextMenuState): ContextEntry[][] => {
        const entry = (scope: string, id: string, run = () => runEditorCommand(id)): ContextEntry => ({ id, label: commandName(scope, id), run });
        const pasteHere = editable && menu.canPaste && entry("app", "paste", () => paste(menu.at));
        const hasNodes = nodes.length > 0;
        const inFrame = selectedNodes.some((n: any) => n.frame);
        const sections =
            menu.target === "edge"
                ? [[entry("node_editor", "delete")]]
                : menu.target === "nodes"
                  ? [
                        [editable && entry("app", "cut", cut), entry("app", "copy", copy), pasteHere, editable && entry("node_editor", "duplicate"), editable && entry("node_editor", "delete")],
                        [editable && entry("node_editor", "join_in_frame"), editable && inFrame && entry("node_editor", "remove_from_frame")],
                        [editable && entry("node_editor", "collapse_all"), editable && entry("node_editor", "expand_all")],
                        [editable && entry("node_editor", "group"), editable && groupCount > 0 && entry("node_editor", "ungroup"), openableCount === 1 && entry("node_editor", "edit_group")],
                    ]
                  : [
                        [editable && entry("node_editor", "add_node", () => openAddMenu(menu.at)), pasteHere],
                        [hasNodes && entry("node_editor", "select_all"), hasNodes && entry("node_editor", "frame_all")],
                        [editable && hasNodes && entry("node_editor", "arrange"), editable && hasNodes && entry("node_editor", "collapse_all"), editable && hasNodes && entry("node_editor", "expand_all")],
                        [!isRoot && entry("node_editor", "exit_group")],
                    ];
        return sections.map((entries) => entries.filter((e): e is ContextEntry => !!e)).filter((entries) => entries.length > 0);
    };
    const contextEntries = contextMenu ? contextSections(contextMenu) : [];

    useOnViewportChange({
        onStart: () => {
            if (grabRef.current) confirmGrab();
            setContextMenu(null);
        },
        // keep where the graph is looked at, coming back out of a group restores it. never an undo step
        onEnd: (viewport) => {
            if (editable) ops.apply([{ op: "viewport", viewport }], { commitToHistory: false }).catch(() => {});
        },
    });

    // connecting to the empty socket on the group input or output adds a group socket, the backend does that
    const onConnect = useCallback(
        (params: Edge | Connection) => {
            if (!editable || linkCancelledRef.current) return;
            // the sockets brought along connect with it when the drag ends
            if (alongRef.current.length > 0) droppedRef.current = params;
            else apply([connectOp(params)]);
        },
        [editable, apply]
    );

    // a single link dragged off a handle and dropped on nothing (the + sign), open the add menu where it was released
    // with the nodes it can connect to
    const onConnectEnd = useCallback(
        (event: MouseEvent | TouchEvent, connectionState: FinalConnectionState) => {
            const along = alongRef.current;
            const dropped = droppedRef.current;
            alongRef.current = [];
            droppedRef.current = null;
            if (linkCancelledRef.current) return;
            // one undo step for the link and the ones brought along
            if (dropped) {
                const output = { node: dropped.target, side: "outputs" as const, socket: dropped.targetHandle ?? "" };
                const input = { node: dropped.source, side: "inputs" as const, socket: dropped.sourceHandle ?? "" };
                const { nodeLookup } = store.getState();
                const inputNode = nodeLookup.get(input.node);
                // outputs dropped on a multi input all go into it
                const multi = !!inputNode && !!inputHandle(lookup, inputNode, input.socket, level.def).multi && along.every((s) => s.side === "outputs");
                const links = connectAlong(nodeLookup, getEdges(), output, input, along, multi);
                const connect = (output: SocketRef, input: SocketRef): Op => ({ op: "connect", from_node: output.node, from_output: output.socket, to_node: input.node, to_input: input.socket });
                let ops = [connectOp(dropped), ...links.map((link) => connect(link.output, link.input))];
                // a multi input gets its links top to bottom so they don't cross, the dragged one where its socket is
                if (multi) {
                    const y = (s: SocketRef) => socketPoints(nodeLookup.get(s.node)!).find((p) => sameSocket(p, s))?.y ?? 0;
                    ops = [output, ...links.map((link) => link.output)].sort((a, b) => y(a) - y(b)).map((o) => connect(o, input));
                }
                apply(ops);
                return;
            }
            // several links dropped on nothing don't add a node
            if (along.length > 0) return;
            const { fromNode, fromHandle } = connectionState;
            if (connectionState.isValid || connectionState.toHandle || !editable || !fromNode || !fromHandle?.id) return;
            const { clientX, clientY } = "changedTouches" in event ? event.changedTouches[0] : event;
            const start = linkStartRef.current;
            if (start && Math.hypot(clientX - start.x, clientY - start.y) <= CLICK_DISTANCE) return;

            // react flow's targets are outputs
            const isOutput = fromHandle.type === "target";
            const handle = (isOutput ? outputHandle : inputHandle)(lookup, fromNode, fromHandle.id, level.def);
            setMenuLink({ nodeId: fromNode.id, handleId: fromHandle.id, isOutput, dataType: handle.data_type });

            // the new node gets placed from this position, same as shift+a
            mousePositionRef.current = { x: clientX, y: clientY };
            setMenuPosition({ x: clientX, y: clientY });
            setMenuOpen(true);
        },
        [editable, lookup, level.def, apply, store, getEdges]
    );

    // the height each node being resized had when the resize started
    const resizeStartRef = useRef(new Map<string, number | undefined>());

    // what react flow changes on screen: selection, drags, sizes. removals, drag ends and resizes are sent as ops.
    // a dragged frame takes the nodes in it along
    const onNodesChange = useCallback(
        (all: any[]) => {
            const changes = keptChanges(all);
            for (const change of changes.filter((c) => c.type === "dimensions" && c.resizing === true && !resizeStartRef.current.has(c.id))) {
                resizeStartRef.current.set(change.id, getNodes().find((n) => n.id === change.id)?.measured?.height);
            }
            const session = moveRef.current;
            const carried = session ? carriedChanges(session, changes) : [];
            const removed = changes.filter((c) => c.type === "remove").map((c) => c.id);
            setNodes((nds) => applyNodeChanges([...changes.filter((c) => c.type !== "remove"), ...carried], nds));
            if (removed.length > 0 && canEdit()) apply([{ op: "delete", nodes: removed }]);
            if (changes.some((c) => c.type === "select")) scheduleSelection();

            // a drag finished, with the nodes the dragged frames carried and the frames nodes were dropped in
            const moved = changes.filter((c) => c.type === "position" && c.dragging === false);
            if (moved.length > 0) {
                const current = new Map(getNodes().map((n) => [n.id, n.position]));
                const positions = Object.fromEntries([...moved, ...carried].map((c) => [c.id, c.position ?? current.get(c.id)]));
                for (const id of session?.carriedBy.keys() ?? []) positions[id] ??= current.get(id);
                const frames = endMove(positions);
                if (canEdit()) apply([moveOp(positions, frames)]);
            }
            // a resize finished, a frame keeps the size it was given
            for (const change of changes.filter((c) => c.type === "dimensions" && c.resizing === false)) {
                const node = getNodes().find((n) => n.id === change.id);
                if (!node || !canEdit()) continue;
                const width = change.dimensions?.width ?? node.width ?? node.measured?.width;
                const height = change.dimensions?.height ?? node.height ?? node.measured?.height;
                if (isFrame(node)) setNodes((nds) => nds.map((n) => (n.id === node.id ? { ...n, data: { ...n.data, shrink: false } } : n)));
                // a viewer without a height of its own keeps growing with what it shows when only its width was dragged. a
                // collapsed node keeps the height it had, it's drawn as tall as its header
                const start = resizeStartRef.current.get(node.id);
                resizeStartRef.current.delete(node.id);
                const stored = (storedRef.current.nodes ?? []).find((n: any) => n.id === node.id);
                const widthOnly = node.type === VIEWER && stored?.height === undefined && start !== undefined && height !== undefined && Math.abs(height - start) < 1;
                const kept = (node.data as any)?.collapsed ? (stored?.height ?? null) : widthOnly ? null : height;
                if (width && height) apply([{ op: "resize", node: node.id, width, height: kept, position: node.position }]);
            }
        },
        [setNodes, canEdit, apply, scheduleSelection, getNodes, endMove]
    );

    const onEdgesChange = useCallback(
        (all: any[]) => {
            const changes = keptChanges(all);
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
                onConnectStart={onConnectStart}
                onConnectEnd={onConnectEnd}
                onSelectionStart={onSelectionStart}
                onSelectionEnd={onSelectionEnd}
                onBoxSockets={onBoxSockets}
                onNodeDragStart={onNodeDragStart}
                onSelectionDragStart={onSelectionDragStart}
                onNodeClick={handleNodeClickStop}
                onNodeDoubleClick={handleNodeDoubleClick}
                onEdgeClick={handleEdgeClick}
                onPaneClick={handlePaneClick}
                onNodeDrag={handleNodeDrag}
                onSelectionDrag={handleNodeDrag}
                onPaneContextMenu={handlePaneContextMenu}
                onNodeContextMenu={handleNodeContextMenu}
                onContextMenu={handleContextMenu}
                onSelectionContextMenu={handleSelectionContextMenu}
                onEdgeContextMenu={handleEdgeContextMenu}
                onInit={onInit}
                isValidConnection={isValidConnection}
                panOnDrag={panHeld ? true : [1]}
                nodesDraggable={editable}
                nodesConnectable={editable}
                className={isRoot ? "" : "group-canvas"}
            />
            <NodeAddMenu isOpen={menuOpen} entries={menuEntries} onClose={closeMenu} onSelect={addNode} position={menuPosition} />
            {tagEdit && <TagMenu key={`${tagEdit.node}\n${tagEdit.side}\n${tagEdit.socket}`} current={currentTag} names={tagNames} position={{ x: tagEdit.x, y: tagEdit.y }} onClose={closeTagMenu} onSubmit={setTag} />}
            {unused && createPortal(<DeleteUnusedModal count={unused.length} onDelete={deleteUnused} onCancel={cancelUnused} />, document.body)}
            {contextMenu && contextEntries.length > 0 && <ContextMenu key={`${contextMenu.at.x},${contextMenu.at.y}`} position={contextMenu.at} sections={contextEntries} onClose={closeContextMenu} />}
        </>
    );
}

export default NodeGraphEditor;
