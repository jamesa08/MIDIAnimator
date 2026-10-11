import { CSSProperties, useCallback, useRef } from "react";
import { Handle, HandleProps, InternalNode, ReactFlowState, useNodeId, useStore } from "@xyflow/react";
import { useGroupContext } from "../../contexts/GroupContext";
import { SocketRef } from "../../utils/graphOps";
import { NEW_SOCKET } from "../../utils/groups";
import { CLICK_DISTANCE } from "./NodeGraphCanvas";

// a socket is selected on its own like an edge (src-tauri/src/graph/sockets.rs): a click on it or its tag, or a box
// around sockets only. the selected ones are tagged together and a link dragged off one of them brings the others

type Side = "inputs" | "outputs";

// window event to open the tag menu on a socket, the node graph editor opens it
export const TAG_EDIT_EVENT = "motionkeys:tag-edit";
export type TagEdit = { scope: string | null; node: string; side: Side; socket: string; x: number; y: number };

// opens the tag menu on a socket of a node in the graph `scope`, at the mouse
export function editTag(scope: string | null, node: string, side: Side, socket: string, event: React.MouseEvent) {
    event.stopPropagation();
    const detail: TagEdit = { scope, node, side, socket, x: event.clientX, y: event.clientY };
    window.dispatchEvent(new CustomEvent(TAG_EDIT_EVENT, { detail }));
}

// window event to select a socket, the node graph editor selects it. `add` keeps the rest of the selection (shift)
export const SOCKET_SELECT_EVENT = "motionkeys:socket-select";
export type SocketSelect = { scope: string | null; node: string; side: Side; socket: string; add: boolean };

// selects a socket of a node in the graph `scope`, not its node
export function selectSocket(scope: string | null, node: string, side: Side, socket: string, event: React.MouseEvent) {
    event.stopPropagation();
    const detail: SocketSelect = { scope, node, side, socket, add: event.shiftKey };
    window.dispatchEvent(new CustomEvent(SOCKET_SELECT_EVENT, { detail }));
}

// window event to pick up the link on a connected input like blender, the node graph editor drags it off. `mouse` is the
// press, react flow's drag has to start from it
export const PICK_UP_EVENT = "motionkeys:pick-up";
export type PickUp = { scope: string | null; node: string; socket: string; mouse: MouseEvent };

// a node keeps which of its sockets are selected
export type SelectedSockets = { inputs: string[]; outputs: string[] };

// the selected sockets of nodes
export const selectedSockets = (nodes: any[]): SocketRef[] => nodes.flatMap((n) => (["inputs", "outputs"] as const).flatMap((side) => ((n.selectedSockets?.[side] ?? []) as string[]).map((socket) => ({ node: n.id, side, socket }))));
export const sameSocket = (a: SocketRef, b: SocketRef) => a.node === b.node && a.side === b.side && a.socket === b.socket;
export const sameSockets = (a: SocketRef[], b: SocketRef[]) => a.length === b.length && a.every((s) => b.some((t) => sameSocket(s, t)));

// whether a socket is selected, for drawing it
export function useSocketSelected(nodeId: string, side: Side, socket: string): boolean {
    const isSelected = useCallback((s: ReactFlowState) => !!(s.nodeLookup.get(nodeId) as any)?.selectedSockets?.[side]?.includes(socket), [nodeId, side, socket]);
    return useStore(isSelected);
}

// react flow's source handles are inputs and its targets outputs
export const handleSide = (type: "source" | "target"): Side => (type === "source" ? "inputs" : "outputs");

// a socket and the middle of its dot, in flow coordinates
export type SocketPoint = SocketRef & { x: number; y: number };

// where each of a node's drawn sockets is. hidden ones (parameters) aren't drawn and the empty socket on the group input
// and output can't be selected
export function socketPoints(node: InternalNode): SocketPoint[] {
    const { x, y } = node.internals.positionAbsolute;
    const handles = [...(node.internals.handleBounds?.source ?? []), ...(node.internals.handleBounds?.target ?? [])];
    return handles.filter((h) => h.id && h.id !== NEW_SOCKET && h.width > 0).map((h) => ({ node: node.id, side: handleSide(h.type), socket: h.id!, x: x + h.x + h.width / 2, y: y + h.y + h.height / 2 }));
}

// the other selected sockets a link dragged off `from` brings along: the ones on the same side, when `from` is selected
export function draggedAlong(nodeLookup: Map<string, InternalNode>, from: SocketRef): SocketPoint[] {
    const selected = (node: InternalNode, side: Side, socket: string) => !!(node.internals.userNode as any).selectedSockets?.[side]?.includes(socket);
    const fromNode = nodeLookup.get(from.node);
    if (!fromNode || !selected(fromNode, from.side, from.socket)) return [];
    return [...nodeLookup.values()].flatMap(socketPoints).filter((p) => p.side === from.side && !sameSocket(p, from) && selected(nodeLookup.get(p.node)!, p.side, p.socket));
}

// react flow only takes presses on a socket that can start a link, one that picks up its link instead still has to
const PICK_UP_STYLE = { pointerEvents: "all", cursor: "crosshair" };

// a socket of a node: react flow's handle, selected by a click that doesn't drag a link off it, a double click opens
// the tag menu. `side` is which of the node's sockets it is
export default function SocketHandle({ side, multi = false, style, ...props }: HandleProps & { id: string; side: Side; multi?: boolean; style?: CSSProperties }) {
    const nodeId = useNodeId() ?? "";
    const { scopeId, editable } = useGroupContext();
    const selected = useSocketSelected(nodeId, side, props.id);
    const pressRef = useRef<{ x: number; y: number } | null>(null);
    const open = props.id === NEW_SOCKET;

    // a connected input doesn't start a new link, a press picks up the one it has (a tag isn't a link to pick up)
    const isConnected = useCallback((s: ReactFlowState) => side === "inputs" && s.edges.some((e: any) => e.source === nodeId && e.sourceHandle === props.id && !e.tagged), [nodeId, side, props.id]);
    const pickUp = useStore(isConnected) && editable && !open;
    // while a link is dragged, a multi input takes it anywhere near the whole socket, react flow only looks near its middle
    const dropZone = useStore((s: ReactFlowState) => s.connection.inProgress) && multi;
    const press = (event: React.MouseEvent) => {
        pressRef.current = { x: event.clientX, y: event.clientY };
        if (!pickUp || event.button !== 0) return;
        const detail: PickUp = { scope: scopeId, node: nodeId, socket: props.id, mouse: event.nativeEvent };
        window.dispatchEvent(new CustomEvent(PICK_UP_EVENT, { detail }));
    };

    // a press that moved further than a click dragged a link off it
    const click = (event: React.MouseEvent) => {
        const press = pressRef.current;
        if (open || (press && Math.hypot(event.clientX - press.x, event.clientY - press.y) > CLICK_DISTANCE)) return;
        selectSocket(scopeId, nodeId, side, props.id, event);
    };

    return (
        <Handle
            {...props}
            isConnectableStart={!pickUp}
            className={[selected && "socket-selected", dropZone && "socket-drop-zone"].filter(Boolean).join(" ") || undefined}
            style={{ ...style, "--socket-color": style?.background, ...(pickUp ? PICK_UP_STYLE : {}) } as CSSProperties}
            onMouseDown={press}
            onClick={click}
            onDoubleClick={editable && !open ? (event) => editTag(scopeId, nodeId, side, props.id, event) : undefined}
        />
    );
}
