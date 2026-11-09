import { invoke } from "@tauri-apps/api/core";
import { memo, useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { BaseEdge, Connection, Edge, EdgeProps, Handle, Node, NodeProps, Position, ReactFlowProvider, applyEdgeChanges, applyNodeChanges, getBezierPath, useReactFlow, useStoreApi } from "@xyflow/react";
import { blockUntilRelease, isHeld, pushModal, useHold, useKeymap, useModal } from "../../utils/keymap";
import { ApplyOptions } from "../../utils/graphOps";
import { socketStyle } from "../../utils/sockets";
import { SOCKET_COLORS, nodeColors } from "../../styles";
import { noteToName } from "../../nodes/assign_notes_to_objects";
import NodeGraphCanvas, { useMultiSelection } from "./NodeGraphCanvas";
import { EdgeRing, lighter } from "./TypedEdge";
import { MapData, MapView, NoteMap, isAnimationLinked, isLinked, mapView, shownAnimations, shownMap, unique } from "../../utils/noteMap";
import ContextMenu, { ContextEntry } from "./ContextMenu";
import { useFrameView } from "./frameView";
import { canvasToolCommands, setEditorState } from "../../utils/editorState";
import { SocketPoint, draggedAlong, handleSide } from "./SocketHandle";
import { connectAlong } from "./connectAlong";

// the note map of Assign Notes to Objects, opened with Tab on the node: MIDI notes and the node's animations wired to the
// object group's objects, drawn as a graph of small nodes that can be selected and moved like the node editor's. an
// object plays each animation wired to it on each of its notes. in rules mode it shows what the rules gave, every
// animation on every object, and the first edit to the wires switches the node to map mode with that as its map. the
// view shows the notes' wires, the animations' or both. keep in sync with assign_notes_to_objects and NoteMap in
// src-tauri/src/graph/executors/animation.rs

// where notes, animations and objects were moved to, by node id. the ones that weren't stay in their column
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

// the map as it's stored: notes sorted, objects and animations without wires left out. the animations are always
// stored, a map without them plays every animation on every object
function cleanMap(map: NoteMap): NoteMap {
    const objects: Record<string, number[]> = {};
    for (const [name, notes] of Object.entries(map.objects)) {
        if (notes.length) objects[name] = unique(notes);
    }
    const animations: Record<string, string[]> = {};
    for (const [name, on] of Object.entries(map.animations)) {
        if (on.length) animations[name] = [...new Set(on)];
    }
    return { objects, notes: unique(map.notes), animations };
}

const copyLists = <T,>(lists: Record<string, T[]>): Record<string, T[]> => Object.fromEntries(Object.entries(lists).map(([name, items]) => [name, [...items]]));
const copyMap = (map: NoteMap): NoteMap => ({ objects: copyLists(map.objects), notes: [...map.notes], animations: copyLists(map.animations) });

// node ids: a note, an animation and an object
const noteKey = (note: number) => `n:${note}`;
const animationKey = (name: string) => `a:${name}`;
const objectKey = (name: string) => `o:${name}`;
const isNote = (id: string) => id.startsWith("n:");
const isAnimation = (id: string) => id.startsWith("a:");
const isObject = (id: string) => id.startsWith("o:");

// MARK: - Nodes and Wires

// notes, animations and objects are small nodes, just a header with the graph's socket on it. like the graph, react
// flow's sources are inputs and its targets outputs: a note's and an animation's socket is an output, an object's an
// input that takes both
const socket = (dataType: string) => ({ width: 14, height: 14, ...socketStyle(dataType) });
// each node's one socket. a selected node's socket is selected, so a wire dragged off it brings the other selected
// nodes' along like the graph's selected sockets. they fill the free sockets from the drop like the graph's, or all
// connect to the socket it's dropped on with Connect All to One held or when it's the only one in reach (takes_all)
const OUT = "out";
const IN = "in";

const NoteNode = memo(({ data }: NodeProps) => (
    // notes the MIDI doesn't play are paler and in italics
    <div className={`note-map-node node${data.added ? " note-map-added" : ""}`} style={nodeColors("midi")}>
        <div className="node-header">
            <span className="flex-1">{noteToName(data.note as number)}</span>
            <span className="note-map-number">{data.note as number}</span>
        </div>
        <Handle type="target" id={OUT} position={Position.Right} style={socket("MIDINote")} />
    </div>
));

// an animation plays on the objects wired to it, its socket in the generator's color. one whose generator isn't
// connected any more is paler and in italics, like the notes the MIDI doesn't play
const AnimationNode = memo(({ data }: NodeProps) => (
    <div className={`note-map-node node${data.stale ? " note-map-added" : ""}`} style={nodeColors("animation")}>
        <div className="node-header">
            <span className="flex-1 truncate">{(data.name as string) || "Animation Generator"}</span>
        </div>
        <Handle type="target" id={OUT} position={Position.Right} style={socket("AnimationGenerator")} />
    </div>
));

// a map node's socket color, an animation's or a note's and an object's
const wireColor = (id: string) => (isAnimation(id) ? SOCKET_COLORS.generator : SOCKET_COLORS.midi);
// the color a wire from a node is drawn in all the way, over the fade between its sockets. an animation's are solid
const wireOverride = (id: string): string | null => (isAnimation(id) ? SOCKET_COLORS.generator : null);

const ObjectNode = memo(({ data }: NodeProps) => (
    <div className="note-map-node node" style={nodeColors("scene")}>
        <div className="node-header">
            <span className="flex-1 truncate">{data.name as string}</span>
        </div>
        <Handle type="source" id={IN} position={Position.Left} style={socket("Array<MIDINote>")} />
    </div>
));

// a wire like the graph's edges, in the color of the sockets it connects
function MapWire({ id, source, target, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, selected, interactionWidth }: EdgeProps) {
    const [path] = getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition });
    // like the graph's edges it fades from one socket's color to the other's when they differ, unless the node it comes
    // from overrides it. react flow's targets are the outputs
    const gradientId = `map-wire-${useId()}`;
    const override = wireOverride(target);
    const from = override ?? wireColor(target);
    const to = override ?? wireColor(source);
    return (
        <>
            {from !== to && (
                <defs>
                    <linearGradient id={gradientId} gradientUnits="userSpaceOnUse" x1={targetX} y1={targetY} x2={sourceX} y2={sourceY}>
                        <stop offset="0%" style={{ stopColor: from }} />
                        <stop offset="100%" style={{ stopColor: to }} />
                    </linearGradient>
                    {selected && (
                        <linearGradient id={`${gradientId}-ring`} gradientUnits="userSpaceOnUse" x1={targetX} y1={targetY} x2={sourceX} y2={sourceY}>
                            <stop offset="0%" style={{ stopColor: lighter(from) }} />
                            <stop offset="100%" style={{ stopColor: lighter(to) }} />
                        </linearGradient>
                    )}
                </defs>
            )}
            {selected && <EdgeRing path={path} stroke={from === to ? lighter(from) : `url(#${gradientId}-ring)`} />}
            <BaseEdge id={id} path={path} interactionWidth={interactionWidth} style={{ stroke: from === to ? from : `url(#${gradientId})` }} />
        </>
    );
}

const nodeTypes = { map_note: NoteNode, map_animation: AnimationNode, map_object: ObjectNode };
const edgeTypes = { default: MapWire };

type Props = {
    node: any;
    info: MapData;
    editable: boolean;
    setInputs: (inputs: Record<string, any>, options?: ApplyOptions) => Promise<unknown>;
    // shows only the notes wired to an object, or all of them. kept on the node, it never runs the graph
    setLinkedOnly: (linkedOnly: boolean) => void;
    // shows the notes' wires, the animations' or both, kept on the node like linked only
    setView: (view: MapView) => void;
    // ends or undoes a transaction (adding a note, then grabbing it until a click places it). undoing says whether it did
    endTxn: (txn: string) => void;
    cancelTxn: (txn: string) => Promise<unknown>;
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

// a note added to the map, and the node's mode and map before it was (adding the first one switches rules to map)
type AddedNote = { note: number; before: { mode?: string; note_map?: any } };

// a grab in progress: the nodes follow the cursor from `cursor` (flow position) until a click places them. `added` the
// note added in its transaction
type Grab = { txn: string | null; cursor: { x: number; y: number }; nodes: { id: string; x: number; y: number }[]; added: AddedNote | null };

// a note being added, on its way to the backend and not grabbed yet: `id` its node, `from` where its grab starts,
// `sent` its edit. `landed` once that's back, `cancelled` when it was cancelled meanwhile, `end` lets go of the modal that
// holds the input until its grab is up
type PendingAdd = { id: string; txn: string; from: { x: number; y: number }; added: AddedNote; sent: Promise<unknown>; landed: boolean; cancelled: boolean; end: () => void };

function NoteMapView(props: Props) {
    return (
        <div className="note-map-layer">
            <ReactFlowProvider>
                <NoteMapCanvas {...props} />
            </ReactFlowProvider>
        </div>
    );
}

function NoteMapCanvas({ node, info, editable, setInputs, setLinkedOnly, setView, endTxn, cancelTxn, onExit, onExitToRoot }: Props) {
    const mode: string = node.data?.inputs?.mode ?? "rules";
    const { objects, midiNotes } = info;
    const layout = useMemo(() => readLayout(node.data?.inputs?.map_layout), [node.data?.inputs?.map_layout]);

    const { map, notes } = useMemo(() => shownMap(mode, info, node.data?.inputs?.note_map), [mode, info, node.data?.inputs?.note_map]);
    const animations = useMemo(() => shownAnimations(map, info), [map, info]);
    // the notes' wires, the animations' or both, with the nodes they come from
    const view = mapView(node.data?.map_view);
    const showNotes = view !== "animations";
    const showAnimations = view !== "notes";

    // with linked only the notes nothing is wired to are hidden. a note shown while it's on stays until it's switched,
    // so removing a note's last wire doesn't take it away under the cursor, and notes the map adds or finds show too
    const linkedOnly = node.data?.linked_only === true;
    const keptRef = useRef(new Set<number>());
    const [keptRev, setKeptRev] = useState(0);
    const visible = useMemo(() => {
        if (!showNotes) return [];
        if (!linkedOnly) {
            keptRef.current = new Set();
            return notes;
        }
        for (const n of notes) if (isLinked(map, objects, n)) keptRef.current.add(n);
        return notes.filter((n) => keptRef.current.has(n));
    }, [showNotes, linkedOnly, notes, map, objects, keptRev]);
    // the animations too, the ones wired to no object are hidden the same way
    const keptAnimationsRef = useRef(new Set<string>());
    const shownAnimationNodes = useMemo(() => {
        if (!showAnimations) return [];
        if (!linkedOnly) {
            keptAnimationsRef.current = new Set();
            return animations;
        }
        for (const a of animations) if (isAnimationLinked(map, objects, a)) keptAnimationsRef.current.add(a);
        return animations.filter((a) => keptAnimationsRef.current.has(a));
    }, [showAnimations, linkedOnly, animations, map, objects]);
    const keep = (ns: number[]) => {
        if (!linkedOnly) return;
        ns.forEach((n) => keptRef.current.add(n));
        setKeptRev((rev) => rev + 1);
    };

    // every edit to the wires stores the whole map and switches the node to map mode, one undo step. a note or an
    // animation that's gone takes its saved position with it
    const commit = useCallback(
        (next: NoteMap, extra: Record<string, any> = {}, options?: ApplyOptions) => {
            if (!editable) return Promise.resolve();
            const stored = cleanMap(next);
            const kept = new Set([...[...midiNotes, ...stored.notes, ...objects.flatMap((o) => stored.objects[o] ?? [])].map(noteKey), ...[...info.animations, ...Object.keys(stored.animations)].map(animationKey)]);
            const saved: Layout = extra.map_layout ?? layout;
            const gone = Object.keys(saved).filter((id) => (isNote(id) || isAnimation(id)) && !kept.has(id));
            if (gone.length) extra = { ...extra, map_layout: Object.fromEntries(Object.entries(saved).filter(([id]) => !gone.includes(id))) };
            return setInputs({ mode: "map", note_map: stored, ...extra }, options);
        },
        [editable, setInputs, midiNotes, objects, info.animations, layout]
    );

    // MARK: - Graph

    // where each shown node sits with `saved` positions: notes in a column on the left with the animations under them,
    // and objects on the right, unless they were moved. moved ones don't take a row, the rest close up, hidden ones too
    const place = useCallback(
        (saved: Layout) => {
            const column = (ids: string[], x: number) => {
                let row = 0;
                return ids.map((id) => {
                    const [lx, ly] = saved[id] ?? [x, row++ * ROW];
                    return [id, { x: lx, y: ly }] as const;
                });
            };
            return new Map([...column([...visible.map(noteKey), ...shownAnimationNodes.map(animationKey)], 0), ...column(objects.map(objectKey), COLUMN)]);
        },
        [visible, shownAnimationNodes, objects]
    );
    // a move stores where the moved nodes went, and where the ones that would close up after them are, so nothing else
    // moves
    const pin = useCallback(
        (next: Layout) => {
            const was = place(layout);
            const pinned = { ...next };
            for (const [id, at] of place(next)) {
                const from = was.get(id)!;
                if (!(id in next) && (at.x !== from.x || at.y !== from.y)) pinned[id] = [from.x, from.y];
            }
            return pinned;
        },
        [place, layout]
    );
    const built = useMemo<Node[]>(() => {
        const at = place(layout);
        return [
            ...visible.map((n) => ({ id: noteKey(n), type: "map_note", position: at.get(noteKey(n))!, data: { note: n, added: !midiNotes.includes(n), takes_all: true } })),
            ...shownAnimationNodes.map((a) => ({ id: animationKey(a), type: "map_animation", position: at.get(animationKey(a))!, data: { name: a, stale: !info.animations.includes(a), takes_all: true } })),
            ...objects.map((o) => ({ id: objectKey(o), type: "map_object", position: at.get(objectKey(o))!, data: { name: info.objectNames[o] ?? o, takes_all: true } })),
        ];
    }, [place, visible, shownAnimationNodes, objects, info.objectNames, midiNotes, info.animations, layout]);
    // the notes' and the animations' wires to the objects, the view's. objects that aren't in the group aren't drawn
    const builtWires = useMemo<Edge[]>(
        () => [
            ...(showNotes ? objects.flatMap((o) => unique(map.objects[o] ?? []).map((n) => ({ id: `${noteKey(n)}>${objectKey(o)}`, source: objectKey(o), sourceHandle: IN, target: noteKey(n), targetHandle: OUT }))) : []),
            ...shownAnimationNodes.flatMap((a) => (map.animations[a] ?? []).filter((o) => objects.includes(o)).map((o) => ({ id: `${animationKey(a)}>${objectKey(o)}`, source: objectKey(o), sourceHandle: IN, target: animationKey(a), targetHandle: OUT }))),
        ],
        [showNotes, objects, map, shownAnimationNodes]
    );

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
    // what's drawn: a selected node's socket is selected too
    const shownNodes = useMemo(() => nodes.map((n) => ({ ...n, selectedSockets: n.selected ? (isObject(n.id) ? { inputs: [IN], outputs: [] } : { inputs: [], outputs: [OUT] }) : undefined })), [nodes]);

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
            setInputs({ map_layout: pin(next) });
        },
        [layout, pin, editable, setInputs]
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

    // a note's or an animation's socket dragged to an object's (or back) connects them. stored like the graph's edges,
    // the source is the object taking them
    const wire = (next: NoteMap, source: string, target: string) => {
        if (!isObject(source)) return;
        const object = source.slice(2);
        if (isNote(target)) {
            const list = (next.objects[object] ??= []);
            const note = Number(target.slice(2));
            if (!list.includes(note)) list.push(note);
        } else if (isAnimation(target)) {
            const list = (next.animations[target.slice(2)] ??= []);
            if (!list.includes(object)) list.push(object);
        }
    };
    // the other selected nodes a wire dragged off a selected node brings along, and that wire once it's dropped on a
    // socket. like the node editor they're all connected to it when the drag ends, one undo step
    const store = useStoreApi();
    const alongRef = useRef<SocketPoint[]>([]);
    const droppedRef = useRef<Connection | null>(null);
    const onConnect = useCallback(
        (connection: Connection) => {
            if (alongRef.current.length > 0) return void (droppedRef.current = connection);
            const next = copyMap(map);
            wire(next, connection.source, connection.target);
            commit(next);
        },
        [map, commit]
    );
    const onConnectEnd = useCallback(() => {
        const along = alongRef.current;
        const dropped = droppedRef.current;
        alongRef.current = [];
        droppedRef.current = null;
        if (!dropped) return;
        const { nodeLookup, edges } = store.getState();
        const output = { node: dropped.target, side: "outputs" as const, socket: OUT };
        const input = { node: dropped.source, side: "inputs" as const, socket: IN };
        const next = copyMap(map);
        wire(next, dropped.source, dropped.target);
        for (const link of connectAlong(nodeLookup, edges, output, input, along, isHeld("node_editor", "connect_to_one"))) wire(next, link.input.node, link.output.node);
        commit(next);
    }, [map, commit, store]);
    const isValidConnection = useCallback((c: Edge | Connection) => isObject(c.source) && (isNote(c.target) || isAnimation(c.target)), []);

    // MARK: - Tools

    const selectedNotes = () => nodes.filter((n) => n.selected && isNote(n.id)).map((n) => n.data.note as number);
    const selectedObjects = () => objects.filter((o) => nodes.some((n) => n.selected && n.id === objectKey(o)));
    const wiredNote = (n: number) => isLinked(map, objects, n);
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
        keep(padded);
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
        keep(ns);
        commit(next);
    };

    // the selected animations (or every connected one) to the selected objects (or every object), next to the wires they
    // have. the toolbar's Assign Animation and the right click menu's
    const selectedAnimations = () => animations.filter((a) => nodes.some((n) => n.selected && n.id === animationKey(a)));
    const assign = () => {
        const as = selectedAnimations().length ? selectedAnimations() : info.animations;
        const os = selectedObjects().length ? selectedObjects() : objects;
        if (!as.length || !os.length) return;
        const next = copyMap(map);
        for (const a of as) next.animations[a] = [...new Set([...(next.animations[a] ?? []), ...os])];
        commit(next);
    };

    // the selected wires and everything connected to the selected notes, animations and objects. added notes that are
    // selected go too, and animations whose generator isn't connected
    const removeSelected = () => {
        const nodeIds = new Set(nodes.filter((n) => n.selected).map((n) => n.id));
        const wireIds = new Set(wires.filter((w) => w.selected).map((w) => w.id));
        if (!nodeIds.size && !wireIds.size) return;
        const next = copyMap(map);
        for (const object of objects) {
            next.objects[object] = (next.objects[object] ?? []).filter((n) => !wireIds.has(`${noteKey(n)}>${objectKey(object)}`) && !nodeIds.has(noteKey(n)) && !nodeIds.has(objectKey(object)));
        }
        next.notes = next.notes.filter((n) => !nodeIds.has(noteKey(n)));
        for (const [a, on] of Object.entries(next.animations)) {
            const key = animationKey(a);
            next.animations[a] = nodeIds.has(key) ? [] : on.filter((o) => !wireIds.has(`${key}>${objectKey(o)}`) && !nodeIds.has(objectKey(o)));
        }
        selectNext.current = new Set();
        commit(next);
    };

    // clears what the view shows: the notes' wires and the added notes, the animations' wires, or both
    const clear = () => {
        if (nodes.some((n) => n.selected) || wires.some((w) => w.selected)) return removeSelected();
        const next = copyMap(map);
        if (showNotes) {
            next.objects = {};
            next.notes = [];
        }
        if (showAnimations) next.animations = {};
        commit(next);
    };

    // MARK: - Grab and Add

    const { screenToFlowPosition } = useReactFlow();
    // framed with the room around it the map first opens with
    const frameView = useFrameView();
    const frame = (nodes?: Node[]) => frameView(nodes, { padding: 0.2 });
    const mouseRef = useRef({ x: 0, y: 0 });
    useEffect(() => {
        const move = (event: MouseEvent) => (mouseRef.current = { x: event.clientX, y: event.clientY });
        window.addEventListener("mousemove", move);
        return () => window.removeEventListener("mousemove", move);
    }, []);
    const cursor = () => screenToFlowPosition(mouseRef.current, { snapToGrid: false });

    const grabRef = useRef<Grab | null>(null);
    const [grabbing, setGrabbing] = useState(false);
    const startGrab = useCallback((grabbed: { id: string; position: { x: number; y: number } }[], from: { x: number; y: number }, txn: string | null, added: AddedNote | null = null) => {
        grabRef.current = { txn, cursor: from, nodes: grabbed.map((n) => ({ id: n.id, x: n.position.x, y: n.position.y })), added };
        setGrabbing(true);
    }, []);

    // undoes an added note. when another step (an MCP edit) ended its transaction first, cancelling it leaves the note,
    // so it's taken out of the map, the map going back to the rules if adding it switched to the map. not when it isn't
    // known whether cancelling undid it all
    const cancelRef = useRef(cancelTxn);
    cancelRef.current = cancelTxn;
    const setInputsRef = useRef(setInputs);
    setInputsRef.current = setInputs;
    const nodeRef = useRef(node);
    nodeRef.current = node;
    const undoAdd = useCallback((txn: string, { note, before }: AddedNote) => {
        cancelRef.current(txn).then(
            (cancelled) => {
                if (cancelled !== false) return;
                const inputs = nodeRef.current.data?.inputs ?? {};
                const stored = inputs.note_map;
                if (!Array.isArray(stored?.notes) || !stored.notes.includes(note)) return;
                const map_layout = Object.fromEntries(Object.entries(inputs.map_layout ?? {}).filter(([id]) => id !== noteKey(note)));
                const back = before.mode !== "map" ? { mode: before.mode ?? null, note_map: before.note_map ?? null } : { note_map: { ...stored, notes: stored.notes.filter((n: number) => n !== note) } };
                setInputsRef.current({ ...back, map_layout });
            },
            (e) => console.error(`history_cancel: ${e}`)
        );
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
        setInputs({ map_layout: pin(next) }, grab.txn ? { txn: grab.txn } : {}).then(() => grab.txn && endTxn(grab.txn));
    }, [layout, pin, setInputs, endTxn]);

    // puts the grabbed nodes back, an added note is undone
    const cancelGrab = useCallback(() => {
        const grab = grabRef.current;
        grabRef.current = null;
        setGrabbing(false);
        if (!grab) return;
        if (grab.txn && grab.added) return undoAdd(grab.txn, grab.added);
        if (grab.txn) return void cancelTxn(grab.txn);
        const start = new Map(grab.nodes.map((n) => [n.id, n]));
        setNodes((current) => current.map((n) => (start.has(n.id) ? { ...n, position: { x: start.get(n.id)!.x, y: start.get(n.id)!.y } } : n)));
    }, [undoAdd, cancelTxn]);

    // like the node editor a grab takes every key and click until it's confirmed or cancelled
    useModal("grab", { confirm: confirmGrab, cancel: cancelGrab }, grabbing);
    // the added note whose grab is starting: its modal is let go once the grab's is up, so nothing gets between them. one
    // cancelled in between is cancelled now
    const handoffRef = useRef<PendingAdd | null>(null);
    useEffect(() => {
        const pending = handoffRef.current;
        if (!grabbing || !pending) return;
        handoffRef.current = null;
        pending.end();
        if (pending.cancelled) cancelGrab();
    }, [grabbing, cancelGrab]);

    // the note being added, until it's grabbed
    const pendingGrab = useRef<PendingAdd | null>(null);

    // leaving the map mid grab undoes the note it was adding, one still on its way is undone once it's back
    useEffect(
        () => () => {
            const grab = grabRef.current;
            if (grab?.txn && grab.added) undoAdd(grab.txn, grab.added);
            else if (grab?.txn) cancelRef.current(grab.txn);
            handoffRef.current?.end();
            handoffRef.current = null;
            const pending = pendingGrab.current;
            pendingGrab.current = null;
            if (!pending) return;
            pending.end();
            pending.sent.then(() => undoAdd(pending.txn, pending.added));
        },
        [undoAdd]
    );

    // Shift+A takes a note number or name, a new note is added under the cursor and grabbed. one already in the map is
    // selected instead
    const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
    const closeMenu = useCallback(() => setMenu(null), []);
    // bumped when an added note's edit is back, whether or not it's drawn
    const [landed, setLanded] = useState(0);
    const addFromMenu = useCallback(
        (note: number) => {
            setMenu(null);
            if (notes.includes(note)) {
                // one hidden by linked only shows again, selected once it's drawn
                if (visible.includes(note)) setNodes((current) => current.map((n) => ({ ...n, selected: n.id === noteKey(note) })));
                else {
                    selectNext.current = new Set([noteKey(note)]);
                    keep([note]);
                }
                setWires((current) => current.map((w) => ({ ...w, selected: false })));
                return;
            }
            const from = cursor();
            const txn = crypto.randomUUID();
            const next = copyMap(map);
            next.notes.push(note);
            selectNext.current = new Set([noteKey(note)]);
            keep([note]);
            // a modal takes every key and click until its grab is up, so nothing ends its transaction first. a cancel
            // meanwhile undoes it once it's back
            const added: AddedNote = { note, before: { mode: node.data?.inputs?.mode, note_map: node.data?.inputs?.note_map } };
            const pending: PendingAdd = { id: noteKey(note), txn, from, added, sent: Promise.resolve(), landed: false, cancelled: false, end: () => {} };
            pending.end = pushModal("grab", { confirm: () => {}, cancel: () => (pending.cancelled = true) });
            pendingGrab.current?.end();
            pendingGrab.current = pending;
            // back, failed or not
            const land = () => {
                pending.landed = true;
                setLanded((n) => n + 1);
            };
            pending.sent = commit(next, { map_layout: { ...layout, [noteKey(note)]: [Math.round(from.x), Math.round(from.y)] } }, { txn }).then(land, land);
        },
        [map, notes, visible, layout, commit, linkedOnly, node]
    );
    // the added note is grabbed once it's drawn, or undone when it was cancelled meanwhile. one that isn't drawn once its
    // edit is back (it failed) has nothing to grab
    useEffect(() => {
        const pending = pendingGrab.current;
        const added = pending && built.find((n) => n.id === pending.id);
        if (!pending || (!added && !pending.landed)) return;
        pendingGrab.current = null;
        if (!added || pending.cancelled) {
            pending.end();
            if (pending.cancelled) pending.sent.then(() => undoAdd(pending.txn, pending.added));
            return;
        }
        startGrab([added], pending.from, pending.txn, pending.added);
        handoffRef.current?.end();
        handoffRef.current = pending;
    }, [builtKey, landed, startGrab, undoAdd]);

    // the toolbar's group buttons only leave the map, nothing is added from its add menu
    useEffect(() => setEditorState({ editable, selected: 0, groups: 0, openable: 0, inGroup: true, entries: null }), [editable]);

    // the node editor's keys, its own commands are off while the map covers it
    useKeymap("node_editor", {
        // notes are added where they're shown
        add_node: () => {
            if (editable && showNotes) setMenu({ ...mouseRef.current });
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
        exit_group: onExit,
        exit_to_root: onExitToRoot,
        frame_all: () => frame(),
        // frames everything when nothing is selected
        frame_selected: () => frame(nodesRef.current.filter((n) => n.selected)),
        ...canvasToolCommands,
    });

    // back to map mode keeps the map the node had, a node without one starts from what the rules give
    const switchMode = (value: string) => {
        if (!editable || value === mode) return;
        if (value === "rules" || node.data?.inputs?.note_map) setInputs({ mode: value });
        else commit(map);
    };

    // MARK: - Canvas

    useMultiSelection();
    const panHeld = useHold("node_editor", "pan");
    // react flow's own drags take every key until the mouse is let go, like the node editor's
    const blockKeys = useCallback(() => blockUntilRelease(), []);
    // a wire being dragged is in the color of the socket it's dragged from
    const [dragColor, setDragColor] = useState<string>(SOCKET_COLORS.midi);
    // in the combined view the type the cursor was last over, a note or an animation (or their wires), stays in front and
    // the other is dimmed. the notes are in front to begin with, objects change nothing
    const [front, setFront] = useState<"notes" | "animations">("notes");
    const bringForward = (id: string) => {
        if (grabRef.current) return;
        if (isNote(id)) setFront("notes");
        else if (isAnimation(id)) setFront("animations");
    };
    const startConnect = useCallback(
        (_: any, { nodeId, handleId, handleType }: { nodeId: string | null; handleId: string | null; handleType: "source" | "target" | null }) => {
            blockKeys();
            setDragColor(wireColor(nodeId ?? ""));
            alongRef.current = nodeId && handleId && handleType ? draggedAlong(store.getState().nodeLookup, { node: nodeId, side: handleSide(handleType), socket: handleId }) : [];
            droppedRef.current = null;
        },
        [blockKeys, store]
    );

    // right click on a node opens its menu, it becomes the selection unless it's in it already. the empty map's menu
    // works on every animation and object, a selection's on what's selected
    const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
    const closeContextMenu = useCallback(() => setContextMenu(null), []);
    const openContextMenu = (event: MouseEvent | React.MouseEvent) => {
        event.preventDefault();
        if (editable) setContextMenu({ x: event.clientX, y: event.clientY });
    };
    const onNodeContextMenu = (event: React.MouseEvent, clicked: Node) => {
        if (!clicked.selected && editable) {
            setNodes((current) => current.map((n) => ({ ...n, selected: n.id === clicked.id })));
            setWires((current) => current.map((w) => ({ ...w, selected: false })));
        }
        openContextMenu(event);
    };
    // named for where it goes, the selected objects or all of them
    const assignLabel = selectedObjects().length ? "Assign Animation To Selected Objects" : "Assign Animation To All Objects";
    const contextSections: ContextEntry[][] = [[{ id: "assign_animation", label: assignLabel, run: assign }]];

    const tool = "px-3 h-6 border border-black bg-white hover:bg-zinc-100";
    // a button that's black while it's on
    const pressed = (on: boolean) => `px-3 h-6 border border-black ${on ? "bg-black text-white" : "bg-white hover:bg-zinc-100"}`;
    const modeButton = (value: string, label: string) => (
        <button className={pressed(mode === value)} onClick={() => switchMode(value)}>
            {label}
        </button>
    );
    const viewButton = (value: MapView, label: string) => (
        <button className={pressed(view === value)} onClick={() => editable && value !== view && setView(value)}>
            {label}
        </button>
    );

    return (
        <>
            <NodeGraphCanvas
                className={`group-canvas${view === "combined" ? ` note-map-front-${front}` : ""}`}
                nodes={shownNodes}
                edges={wires}
                nodeTypes={nodeTypes}
                edgeTypes={edgeTypes}
                connectionLineStyle={{ stroke: dragColor }}
                onNodesChange={onNodesChange}
                onEdgesChange={onEdgesChange}
                onConnect={onConnect}
                onConnectEnd={onConnectEnd}
                isValidConnection={isValidConnection}
                onConnectStart={startConnect}
                onSelectionStart={blockKeys}
                onNodeDragStart={blockKeys}
                onSelectionDragStart={blockKeys}
                onNodeContextMenu={onNodeContextMenu}
                onNodeMouseEnter={(_, entered) => bringForward(entered.id)}
                onEdgeMouseEnter={(_, wire) => bringForward(wire.id)}
                onSelectionContextMenu={openContextMenu}
                onPaneContextMenu={openContextMenu}
                onContextMenu={(event) => event.preventDefault()}
                panOnDrag={panHeld ? true : [1]}
                nodesDraggable={editable}
                nodesConnectable={editable}
                fitView
                fitViewOptions={{ maxZoom: 1, padding: 0.2 }}
            />
            {menu && <NoteAddBox position={menu} onAdd={addFromMenu} onClose={closeMenu} />}
            {contextMenu && <ContextMenu position={contextMenu} sections={contextSections} onClose={closeContextMenu} />}
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
            {/* a second row: what the map shows, and the animations' tool */}
            <div className="graph-overlay absolute top-16 z-10 flex items-center gap-1 h-6 font-[Arial,sans-serif] text-xs select-none">
                <div className="flex">
                    {viewButton("notes", "Notes")}
                    {viewButton("animations", "Animations")}
                    {viewButton("combined", "Combined")}
                </div>
                <button className={`${pressed(linkedOnly)} ml-2`} onClick={() => editable && setLinkedOnly(!linkedOnly)}>
                    Linked Only
                </button>
                <button className={`${tool} ml-2`} onClick={assign}>
                    {assignLabel}
                </button>
            </div>
        </>
    );
}

export default NoteMapView;
