import { invoke } from "@tauri-apps/api/core";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { BaseEdge, Connection, Edge, EdgeProps, Handle, Node, NodeProps, Position, ReactFlowProvider, applyEdgeChanges, applyNodeChanges, getBezierPath, useReactFlow, useStore, useStoreApi } from "@xyflow/react";
import { blockUntilRelease, useHold, useKeymap, useModal } from "../../utils/keymap";
import { ApplyOptions } from "../../utils/graphOps";
import { socketStyle } from "../../utils/sockets";
import { SOCKET_COLORS, nodeColors } from "../../styles";
import { noteToName } from "../../nodes/assign_notes_to_objects";
import NodeGraphCanvas, { useShiftMultiSelection } from "./NodeGraphCanvas";
import { EdgeRing, lighter } from "./TypedEdge";
import { MapData, NoteMap, shownMap, unique } from "../../utils/noteMap";

// the note map of Assign Notes to Objects, opened with Tab on the node: MIDI notes wired to the object group's objects,
// drawn as a graph of small nodes that can be selected and moved like the node editor's. in rules mode it shows what the
// rules gave, the first edit to the wires switches the node to map mode with that as its map. keep in sync with
// assign_notes_to_objects and NoteMap in src-tauri/src/graph/executors/animation.rs

// where notes and objects were moved to, by node id. the ones that weren't stay in their column
type Layout = Record<string, [number, number]>;

const ROW = 32;
const COLUMN = 400;

function readLayout(value: any): Layout {
    const layout: Layout = {};
    for (const [id, at] of Object.entries(value ?? {})) {
        if (Array.isArray(at) && at.length === 2 && at.every((v) => typeof v === "number")) layout[id] = at as [number, number];
    }
    return layout;
}

// the map as it's stored: notes sorted, objects without notes left out
function cleanMap(map: NoteMap): NoteMap {
    const objects: Record<string, number[]> = {};
    for (const [name, notes] of Object.entries(map.objects)) {
        if (notes.length) objects[name] = unique(notes);
    }
    return { objects, notes: unique(map.notes) };
}

const copyMap = (map: NoteMap): NoteMap => ({ objects: Object.fromEntries(Object.entries(map.objects).map(([name, notes]) => [name, [...notes]])), notes: [...map.notes] });

// node ids: a note and an object
const noteKey = (note: number) => `n:${note}`;
const objectKey = (name: string) => `o:${name}`;
const isNote = (id: string) => id.startsWith("n:");
const isObject = (id: string) => id.startsWith("o:");

// MARK: - Nodes and Wires

// notes and objects are small nodes, just a header with the graph's socket on it. like the graph, react flow's sources
// are inputs and its targets outputs: a note's socket is an output, an object's an input
const socket = (dataType: string) => ({ width: 14, height: 14, ...socketStyle(dataType) });

const NoteNode = memo(({ data }: NodeProps) => (
    // notes the MIDI doesn't play are paler and in italics
    <div className={`note-map-node node${data.added ? " note-map-added" : ""}`} style={nodeColors("midi")}>
        <div className="node-header">
            <span className="flex-1">{noteToName(data.note as number)}</span>
            <span className="note-map-number">{data.note as number}</span>
        </div>
        <Handle type="target" position={Position.Right} style={socket("MIDINote")} />
    </div>
));

const ObjectNode = memo(({ data }: NodeProps) => (
    <div className="note-map-node node" style={nodeColors("scene")}>
        <div className="node-header">
            <span className="flex-1 truncate">{data.name as string}</span>
        </div>
        <Handle type="source" position={Position.Left} style={socket("Array<MIDINote>")} />
    </div>
));

// a wire like the graph's edges, in the notes' socket color
function MapWire({ id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, selected, interactionWidth }: EdgeProps) {
    const [path] = getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition });
    return (
        <>
            {selected && <EdgeRing path={path} stroke={lighter(SOCKET_COLORS.midi)} />}
            <BaseEdge id={id} path={path} interactionWidth={interactionWidth} style={{ stroke: SOCKET_COLORS.midi }} />
        </>
    );
}

// whether a wire's path passes through a box (flow coordinates), sampled along the path as it's drawn
function wireCrosses(root: Element | null | undefined, id: string, box: { x0: number; y0: number; x1: number; y1: number }): boolean {
    const path = root?.querySelector<SVGPathElement>(`.react-flow__edge[data-id="${CSS.escape(id)}"] .react-flow__edge-path`);
    if (!path) return false;
    const length = path.getTotalLength();
    for (let at = 0; at <= length; at += 4) {
        const p = path.getPointAtLength(at);
        if (p.x >= box.x0 && p.x <= box.x1 && p.y >= box.y0 && p.y <= box.y1) return true;
    }
    return false;
}

const nodeTypes = { map_note: NoteNode, map_object: ObjectNode };
const edgeTypes = { default: MapWire };

type Props = {
    node: any;
    info: MapData;
    editable: boolean;
    setInputs: (inputs: Record<string, any>, options?: ApplyOptions) => Promise<unknown>;
    // ends or undoes a transaction (adding a note, then grabbing it until a click places it)
    endTxn: (txn: string) => void;
    cancelTxn: (txn: string) => void;
    onExit: () => void;
    onExitToRoot: () => void;
};

// a note typed as its number (60) or its name like noteToName gives it (C3, c#3), flats too (Db3). null when it isn't one
export function parseNote(text: string): number | null {
    const typed = text.trim();
    if (/^\d+$/.test(typed)) return Number(typed) <= 127 ? Number(typed) : null;
    const name = typed.match(/^([a-gA-G])([#b]?)(-?\d+)$/);
    if (!name) return null;
    const base = { c: 0, d: 2, e: 4, f: 5, g: 7, a: 9, b: 11 }[name[1].toLowerCase() as "c"];
    const note = (Number(name[3]) + 2) * 12 + base + (name[2] === "#" ? 1 : name[2] === "b" ? -1 : 0);
    return note >= 0 && note <= 127 ? note : null;
}

// Shift+A's box: a note typed in and Enter adds it, Escape or a click anywhere else closes it
function NoteAddBox({ position, onAdd, onClose }: { position: { x: number; y: number }; onAdd: (note: number) => void; onClose: () => void }) {
    const [text, setText] = useState("");
    const boxRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);
    useEffect(() => inputRef.current?.focus(), []);
    useEffect(() => {
        const close = (event: MouseEvent) => {
            if (!boxRef.current?.contains(event.target as globalThis.Node)) onClose();
        };
        window.addEventListener("mousedown", close, true);
        return () => window.removeEventListener("mousedown", close, true);
    }, [onClose]);

    // takes the keys while it's open, typing still goes to the box
    useModal(
        "add_menu",
        {
            confirm: () => {
                const note = parseNote(text);
                if (note !== null) onAdd(note);
            },
            cancel: onClose,
        },
        true,
        { passthrough: true }
    );

    // on the body like the node editor's add menu, the canvas is its own stacking context
    return createPortal(
        <div ref={boxRef} style={{ position: "fixed", left: position.x, top: position.y }} className="note-add-box bg-[#2a2a2a] border border-[#444] rounded w-[160px] z-[1000] flex flex-col">
            <input ref={inputRef} type="text" value={text} onChange={(e) => setText(e.target.value)} className="px-2 py-1 bg-[#1a1a1a] border-0 text-white outline-none text-[13px] rounded" />
        </div>,
        document.body
    );
}

// a grab in progress: the nodes follow the cursor from `cursor` (flow position) until a click places them
type Grab = { txn: string | null; cursor: { x: number; y: number }; nodes: { id: string; x: number; y: number }[] };

function NoteMapView(props: Props) {
    return (
        <div className="note-map-layer">
            <ReactFlowProvider>
                <NoteMapCanvas {...props} />
            </ReactFlowProvider>
        </div>
    );
}

function NoteMapCanvas({ node, info, editable, setInputs, endTxn, cancelTxn, onExit, onExitToRoot }: Props) {
    const mode: string = node.data?.inputs?.mode ?? "rules";
    const { objects, midiNotes } = info;
    const layout = useMemo(() => readLayout(node.data?.inputs?.map_layout), [node.data?.inputs?.map_layout]);

    const { map, notes } = useMemo(() => shownMap(mode, info, node.data?.inputs?.note_map), [mode, info, node.data?.inputs?.note_map]);

    // every edit to the wires stores the whole map and switches the node to map mode, one undo step. a note that's gone
    // takes its saved position with it
    const commit = useCallback(
        (next: NoteMap, extra: Record<string, any> = {}, options?: ApplyOptions) => {
            if (!editable) return Promise.resolve();
            const kept = new Set([...midiNotes, ...next.notes, ...objects.flatMap((o) => next.objects[o] ?? [])].map(noteKey));
            const saved: Layout = extra.map_layout ?? layout;
            const gone = Object.keys(saved).filter((id) => isNote(id) && !kept.has(id));
            if (gone.length) extra = { ...extra, map_layout: Object.fromEntries(Object.entries(saved).filter(([id]) => !gone.includes(id))) };
            return setInputs({ mode: "map", note_map: cleanMap(next), ...extra }, options);
        },
        [editable, setInputs, midiNotes, objects, layout]
    );

    // MARK: - Graph

    // notes in a column on the left and objects on the right, unless they were moved. moved ones don't take a row, the
    // rest close up
    const column = (ids: string[], x: number) => {
        let row = 0;
        return ids.map((id) => {
            const [lx, ly] = layout[id] ?? [x, row++ * ROW];
            return { x: lx, y: ly };
        });
    };
    const built = useMemo<Node[]>(() => {
        const notesAt = column(notes.map(noteKey), 0);
        const objectsAt = column(objects.map(objectKey), COLUMN);
        return [...notes.map((n, i) => ({ id: noteKey(n), type: "map_note", position: notesAt[i], data: { note: n, added: !midiNotes.includes(n) } })), ...objects.map((o, i) => ({ id: objectKey(o), type: "map_object", position: objectsAt[i], data: { name: o } }))];
    }, [notes, objects, midiNotes, layout]);
    const builtWires = useMemo<Edge[]>(() => objects.flatMap((o) => unique(map.objects[o] ?? []).map((n) => ({ id: `${noteKey(n)}>${objectKey(o)}`, source: objectKey(o), target: noteKey(n) }))), [objects, map]);

    // react flow's copies, they keep the selection and positions mid drag. a tool can ask for what's selected after its edit
    const [nodes, setNodes] = useState<Node[]>(built);
    const [wires, setWires] = useState<Edge[]>(builtWires);
    const selectNext = useRef<Set<string> | null>(null);
    // every run sends the node's inputs and results again as new objects, only a real change to the notes, objects or
    // wires rebuilds them
    const builtKey = useMemo(() => JSON.stringify(built), [built]);
    const wiresKey = useMemo(() => JSON.stringify(builtWires), [builtWires]);
    const builtRef = useRef(built);
    builtRef.current = built;
    const builtWiresRef = useRef(builtWires);
    builtWiresRef.current = builtWires;
    useEffect(() => {
        setNodes((current) => {
            const was = new Map(current.map((n) => [n.id, n]));
            const selected = selectNext.current ?? new Set(current.filter((n) => n.selected).map((n) => n.id));
            selectNext.current = null;
            // keeps the sizes react flow measured, a node handed over without one is hidden until it's measured again,
            // which rebuilds in quick succession never gave it time to do
            return builtRef.current.map((n) => ({ ...n, measured: was.get(n.id)?.measured, selected: selected.has(n.id) }));
        });
    }, [builtKey]);
    useEffect(() => {
        setWires((current) => {
            const selected = new Set(current.filter((w) => w.selected).map((w) => w.id));
            return builtWiresRef.current.map((w) => ({ ...w, selected: selected.has(w.id) }));
        });
    }, [wiresKey]);
    const nodesRef = useRef(nodes);
    nodesRef.current = nodes;

    // a drag that finished stores where the moved nodes are, in either mode (it doesn't change the map)
    const onNodesChange = useCallback(
        (changes: any[]) => {
            setNodes((current) =>
                applyNodeChanges(
                    changes.filter((c) => c.type !== "remove"),
                    current
                )
            );
            const moved = changes.filter((c) => c.type === "position" && c.dragging === false);
            if (!moved.length || !editable) return;
            const positions = new Map(nodesRef.current.map((n) => [n.id, n.position]));
            const next = { ...layout };
            for (const change of moved) {
                const p = change.position ?? positions.get(change.id);
                if (p) next[change.id] = [Math.round(p.x), Math.round(p.y)];
            }
            setInputs({ map_layout: next });
        },
        [layout, editable, setInputs]
    );
    const onEdgesChange = useCallback(
        (changes: any[]) =>
            setWires((current) =>
                applyEdgeChanges(
                    changes.filter((c) => c.type !== "remove"),
                    current
                )
            ),
        []
    );

    // a note's socket dragged to an object's (or back) connects them. stored like the graph's edges, the source is the
    // object taking the note
    const onConnect = useCallback(
        ({ source, target }: Connection) => {
            if (!isObject(source) || !isNote(target)) return;
            const next = copyMap(map);
            const list = (next.objects[source.slice(2)] ??= []);
            const note = Number(target.slice(2));
            if (!list.includes(note)) list.push(note);
            commit(next);
        },
        [map, commit]
    );
    const isValidConnection = useCallback((c: Edge | Connection) => isObject(c.source) && isNote(c.target), []);

    // box select takes the wires the box crosses too, not only the ones on the notes and objects inside it (react flow's)
    const store = useStoreApi();
    const selectionRect = useStore((s) => s.userSelectionRect);
    useEffect(() => {
        if (!selectionRect) return;
        const { transform, domNode } = store.getState();
        const [tx, ty, zoom] = transform;
        const box = { x0: (selectionRect.x - tx) / zoom, y0: (selectionRect.y - ty) / zoom, x1: (selectionRect.x + selectionRect.width - tx) / zoom, y1: (selectionRect.y + selectionRect.height - ty) / zoom };
        const inside = new Set(nodesRef.current.filter((n) => n.selected).map((n) => n.id));
        setWires((current) =>
            current.map((w) => {
                const selected = inside.has(w.source) || inside.has(w.target) || wireCrosses(domNode, w.id, box);
                return selected === !!w.selected ? w : { ...w, selected };
            })
        );
    }, [selectionRect, store]);

    // MARK: - Tools

    const selectedNotes = () => nodes.filter((n) => n.selected && isNote(n.id)).map((n) => n.data.note as number);
    const selectedObjects = () => objects.filter((o) => nodes.some((n) => n.selected && n.id === objectKey(o)));
    const wiredNote = (n: number) => objects.some((o) => map.objects[o]?.includes(n));
    const wiredObject = (o: string) => !!map.objects[o]?.length;
    const [listText, setListText] = useState("");
    const listNotes = () =>
        listText
            .split(/[\s,[\]]+/)
            .map(parseNote)
            .filter((n): n is number => n !== null);

    const connect = (next: NoteMap, ns: number[], os: string[]) => {
        ns.forEach((note, i) => {
            if (i >= os.length) return;
            const list = (next.objects[os[i]] ??= []);
            if (!list.includes(note)) list.push(note);
        });
    };

    // the selected notes to the selected objects in order, or every note with nothing connected to every object without notes
    // with a selection the wires between its notes and objects are replaced, so they're paired again in order
    const inOrder = () => {
        const selected = selectedNotes().length > 0 && selectedObjects().length > 0;
        const ns = selectedNotes().length ? selectedNotes() : notes.filter((n) => !wiredNote(n));
        const os = selectedObjects().length ? selectedObjects() : objects.filter((o) => !wiredObject(o));
        const next = copyMap(map);
        if (selected) for (const o of os) next.objects[o] = (next.objects[o] ?? []).filter((n) => !ns.includes(n));
        connect(next, ns, os);
        selectNext.current = new Set();
        commit(next);
    };

    // pads the notes out to one per selected object (or every object) like the rules do (pad_nums): the gaps first, then
    // below and above in turn. selects them with the objects for Auto Connect In Order
    const padRange = async () => {
        const base = selectedNotes().length ? selectedNotes() : notes;
        const os = selectedObjects().length ? selectedObjects() : objects;
        if (!base.length || base.length >= os.length) return;
        const padded = await invoke<number[]>("pad_note_numbers", { notes: base, amount: os.length });
        const next = copyMap(map);
        next.notes.push(...padded.filter((n) => !notes.includes(n)));
        selectNext.current = new Set([...padded.map(noteKey), ...os.map(objectKey)]);
        commit(next);
    };

    // the typed notes to the selected objects (or every object) in order, adding the notes that aren't there
    const fromList = () => {
        const ns = listNotes();
        if (!ns.length) return;
        const os = selectedObjects().length ? selectedObjects() : objects;
        const next = copyMap(map);
        next.notes.push(...ns.filter((n) => !notes.includes(n)));
        connect(next, ns, os);
        selectNext.current = new Set();
        commit(next);
    };

    // the selected wires and everything connected to the selected notes and objects, added notes that are selected go too
    const removeSelected = () => {
        const nodeIds = new Set(nodes.filter((n) => n.selected).map((n) => n.id));
        const wireIds = new Set(wires.filter((w) => w.selected).map((w) => w.id));
        if (!nodeIds.size && !wireIds.size) return;
        const next = copyMap(map);
        for (const object of objects) {
            next.objects[object] = (next.objects[object] ?? []).filter((n) => !wireIds.has(`${noteKey(n)}>${objectKey(object)}`) && !nodeIds.has(noteKey(n)) && !nodeIds.has(objectKey(object)));
        }
        next.notes = next.notes.filter((n) => !nodeIds.has(noteKey(n)));
        selectNext.current = new Set();
        commit(next);
    };

    const clear = () => {
        if (nodes.some((n) => n.selected) || wires.some((w) => w.selected)) removeSelected();
        else commit({ objects: {}, notes: [] });
    };

    // MARK: - Grab and Add

    const { screenToFlowPosition } = useReactFlow();
    const mouseRef = useRef({ x: 0, y: 0 });
    useEffect(() => {
        const move = (event: MouseEvent) => (mouseRef.current = { x: event.clientX, y: event.clientY });
        window.addEventListener("mousemove", move);
        return () => window.removeEventListener("mousemove", move);
    }, []);
    const cursor = () => screenToFlowPosition(mouseRef.current, { snapToGrid: false });

    const grabRef = useRef<Grab | null>(null);
    const [grabbing, setGrabbing] = useState(false);
    const startGrab = useCallback((grabbed: { id: string; position: { x: number; y: number } }[], from: { x: number; y: number }, txn: string | null) => {
        grabRef.current = { txn, cursor: from, nodes: grabbed.map((n) => ({ id: n.id, x: n.position.x, y: n.position.y })) };
        setGrabbing(true);
    }, []);

    useEffect(() => {
        if (!grabbing) return;
        const move = (event: MouseEvent) => {
            const grab = grabRef.current;
            if (!grab) return;
            const p = screenToFlowPosition({ x: event.clientX, y: event.clientY }, { snapToGrid: false });
            const start = new Map(grab.nodes.map((n) => [n.id, n]));
            setNodes((current) => current.map((n) => (start.has(n.id) ? { ...n, position: { x: start.get(n.id)!.x + p.x - grab.cursor.x, y: start.get(n.id)!.y + p.y - grab.cursor.y } } : n)));
        };
        window.addEventListener("mousemove", move);
        return () => window.removeEventListener("mousemove", move);
    }, [grabbing, screenToFlowPosition]);

    // places the grabbed nodes where they are, ending the add it belongs to
    const confirmGrab = useCallback(() => {
        const grab = grabRef.current;
        grabRef.current = null;
        setGrabbing(false);
        if (!grab) return;
        const placed = new Map(nodesRef.current.map((n) => [n.id, n.position]));
        const next = { ...layout };
        for (const { id } of grab.nodes) {
            const p = placed.get(id);
            if (p) next[id] = [Math.round(p.x), Math.round(p.y)];
        }
        setInputs({ map_layout: next }, grab.txn ? { txn: grab.txn } : {}).then(() => grab.txn && endTxn(grab.txn));
    }, [layout, setInputs, endTxn]);

    // puts the grabbed nodes back, an added note is undone
    const cancelGrab = useCallback(() => {
        const grab = grabRef.current;
        grabRef.current = null;
        setGrabbing(false);
        if (!grab) return;
        if (grab.txn) return cancelTxn(grab.txn);
        const start = new Map(grab.nodes.map((n) => [n.id, n]));
        setNodes((current) => current.map((n) => (start.has(n.id) ? { ...n, position: { x: start.get(n.id)!.x, y: start.get(n.id)!.y } } : n)));
    }, [cancelTxn]);

    // like the node editor a grab takes every key and click until it's confirmed or cancelled
    useModal("grab", { confirm: confirmGrab, cancel: cancelGrab }, grabbing);

    // leaving the map mid grab undoes the note it was adding
    const cancelRef = useRef(cancelTxn);
    cancelRef.current = cancelTxn;
    useEffect(
        () => () => {
            const txn = grabRef.current?.txn;
            if (txn) cancelRef.current(txn);
        },
        []
    );

    // Shift+A takes a note number or name, a new note is added under the cursor and grabbed. one already in the map is
    // selected instead
    const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
    const closeMenu = useCallback(() => setMenu(null), []);
    const pendingGrab = useRef<{ id: string; txn: string; from: { x: number; y: number } } | null>(null);
    const addFromMenu = useCallback(
        (note: number) => {
            setMenu(null);
            if (notes.includes(note)) {
                setNodes((current) => current.map((n) => ({ ...n, selected: n.id === noteKey(note) })));
                setWires((current) => current.map((w) => ({ ...w, selected: false })));
                return;
            }
            const from = cursor();
            const txn = crypto.randomUUID();
            const next = copyMap(map);
            next.notes.push(note);
            pendingGrab.current = { id: noteKey(note), txn, from };
            selectNext.current = new Set([noteKey(note)]);
            commit(next, { map_layout: { ...layout, [noteKey(note)]: [Math.round(from.x), Math.round(from.y)] } }, { txn });
        },
        [map, notes, layout, commit]
    );
    // the added note is grabbed once it's drawn
    useEffect(() => {
        const pending = pendingGrab.current;
        const added = pending && built.find((n) => n.id === pending.id);
        if (!pending || !added) return;
        pendingGrab.current = null;
        startGrab([added], pending.from, pending.txn);
    }, [builtKey, startGrab]);

    // the node editor's keys, its own commands are off while the map covers it
    useKeymap("node_editor", {
        add_node: () => {
            if (editable) setMenu({ ...mouseRef.current });
        },
        grab: () => {
            const selected = nodesRef.current.filter((n) => n.selected);
            if (editable && selected.length) startGrab(selected, cursor(), null);
        },
        delete: removeSelected,
        // deselect everything if anything is selected, otherwise select every note and object
        select_all: () => {
            const any = nodesRef.current.some((n) => n.selected) || wires.some((w) => w.selected);
            setNodes((current) => current.map((n) => ({ ...n, selected: !any })));
            setWires((current) => current.map((w) => ({ ...w, selected: false })));
        },
        edit_group: onExit,
        exit_to_root: onExitToRoot,
    });

    // back to map mode keeps the map the node had, a node without one starts from what the rules give
    const switchMode = (value: string) => {
        if (!editable || value === mode) return;
        if (value === "rules" || node.data?.inputs?.note_map) setInputs({ mode: value });
        else commit(map);
    };

    // MARK: - Canvas

    useShiftMultiSelection();
    const panHeld = useHold("node_editor", "pan");
    // react flow's own drags take every key until the mouse is let go, like the node editor's
    const blockKeys = useCallback(() => blockUntilRelease(), []);

    const tool = "px-3 h-6 border border-black bg-white hover:bg-zinc-100";
    const modeButton = (value: string, label: string) => (
        <button className={`px-3 h-6 border border-black ${mode === value ? "bg-black text-white" : "bg-white hover:bg-zinc-100"}`} onClick={() => switchMode(value)}>
            {label}
        </button>
    );

    return (
        <>
            <NodeGraphCanvas
                className="group-canvas"
                nodes={nodes}
                edges={wires}
                nodeTypes={nodeTypes}
                edgeTypes={edgeTypes}
                onNodesChange={onNodesChange}
                onEdgesChange={onEdgesChange}
                onConnect={onConnect}
                isValidConnection={isValidConnection}
                onConnectStart={blockKeys}
                onSelectionStart={blockKeys}
                onNodeDragStart={blockKeys}
                onSelectionDragStart={blockKeys}
                panOnDrag={panHeld ? true : [1]}
                nodesDraggable={editable}
                nodesConnectable={editable}
                fitView
                fitViewOptions={{ maxZoom: 1, padding: 0.2 }}
            />
            {menu && <NoteAddBox position={menu} onAdd={addFromMenu} onClose={closeMenu} />}
            <div className="graph-overlay absolute top-9 z-10 flex items-center gap-1 h-6 font-[Arial,sans-serif] text-xs select-none">
                <div className="flex">
                    {modeButton("rules", "Rules")}
                    {modeButton("map", "Map")}
                </div>
                <button className={`${tool} ml-2`} onClick={inOrder}>
                    Auto Connect In Order
                </button>
                <button className={tool} onClick={padRange}>
                    Pad Range
                </button>
                <input className="h-6 w-24 px-1 border border-black bg-white" value={listText} onChange={(e) => setListText(e.target.value)} />
                <button className={tool} onClick={fromList}>
                    From List
                </button>
                <button className={tool} onClick={clear}>
                    Clear
                </button>
            </div>
        </>
    );
}

export default NoteMapView;
