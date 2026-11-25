import { createContext } from "react";
import { Edge, InternalNode, Node } from "@xyflow/react";
import { COLUMN_GAP } from "../../utils/arrange";
import { Rect, isFrame, nodeRect } from "../../utils/frames";
import { GroupDef, SpecLookup, inputHandle, outputHandle } from "../../utils/groups";
import { linkRank } from "../../utils/sockets";
import { socketPoints } from "./SocketHandle";
import { edgeCrosses, edgeDistance, edgePaths } from "./edgeGeometry";

// a node dragged or grabbed onto a link goes into it when it's dropped, like blender: the link's output feeds the node
// and the node feeds the link's input. it and the nodes after it glide right to make room
type XY = { x: number; y: number };

// the link the dragged node would go into, ringed like a selected one (TypedEdge)
export const InsertLinkContext = createContext<string | null>(null);

// only a single node with nothing connected to it (a tag connects it too) goes into a link, not a frame
export const insertable = (node: Node | InternalNode | undefined, edges: Edge[]) => !!node && !isFrame(node) && !edges.some((e) => e.source === node.id || e.target === node.id);

// the link a node at `rect` covers that comes nearest `cursor` (flow coordinates), not a tag's
export function linkUnder(root: Element | null | undefined, edges: Edge[], rect: Rect, cursor: XY): Edge | null {
    const box = { x0: rect.x, y0: rect.y, x1: rect.x + rect.width, y1: rect.y + rect.height };
    const paths = edgePaths(root);
    let best: Edge | null = null;
    let bestDistance = Infinity;
    for (const edge of edges) {
        const path = paths.get(edge.id);
        if (!path || edge.hidden || (edge as any).tagged || !edgeCrosses(path, box)) continue;
        const distance = edgeDistance(path, cursor);
        if (distance < bestDistance) [best, bestDistance] = [edge, distance];
    }
    return best;
}

// the node's socket on `side` that best takes `type` (linkRank, like a node added from a link), the top one of those that
// take it as well
function bestSocket(lookup: SpecLookup, scope: GroupDef | null, node: InternalNode, side: "inputs" | "outputs", type: string): string | null {
    const rows = socketPoints(node)
        .filter((p) => p.side === side)
        .sort((a, b) => a.y - b.y);
    let best: string | null = null;
    let bestRank = Infinity;
    for (const { socket } of rows) {
        let r: number;
        if (side === "inputs") r = linkRank(type, inputHandle(lookup, node, socket, scope).data_type);
        else {
            // dynamic outputs only show what was made, they can't be connected
            const spec = lookup(node, scope)?.handles.outputs.find((h) => h.id === socket);
            r = !spec || spec.data_type.startsWith("Dyn<") ? Infinity : linkRank(outputHandle(lookup, node, socket, scope).data_type, type);
        }
        if (r < bestRank) [best, bestRank] = [socket, r];
    }
    return best;
}

// the node's input the link's output goes into and its output that goes into the link's input, null when either has none
// that takes it. stored edges are reversed: `target` is the node giving the value, `source` the one taking it
export function insertSockets(lookup: SpecLookup, scope: GroupDef | null, nodeLookup: Map<string, InternalNode>, node: InternalNode, edge: Edge): { input: string; output: string } | null {
    const [from, to] = [nodeLookup.get(edge.target), nodeLookup.get(edge.source)];
    if (!from || !to) return null;
    const input = bestSocket(lookup, scope, node, "inputs", outputHandle(lookup, from, edge.targetHandle ?? "", scope).data_type);
    const output = bestSocket(lookup, scope, node, "outputs", inputHandle(lookup, to, edge.sourceHandle ?? "", scope).data_type);
    return input && output ? { input, output } : null;
}

// where the nodes go to make room for a node at `rect` put into a link from `prev` into `next` (blender's auto offset):
// the node moves right until it's COLUMN_GAP clear of prev, then when next is closer than that to its right, next and
// the nodes it feeds on the right of where the node was dropped move right until it isn't
export function makeRoom(nodes: Node[], edges: Edge[], inserted: string, rect: Rect, prev: string, next: string): Record<string, XY> {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const room: Record<string, XY> = {};
    const middle = rect.x + rect.width / 2;
    const prevNode = byId.get(prev);
    let x = rect.x;
    if (prevNode && !isFrame(prevNode)) {
        const from = nodeRect(prevNode);
        if (middle > from.x + from.width / 2 && x < from.x + from.width + COLUMN_GAP) x = from.x + from.width + COLUMN_GAP;
    }
    if (x !== rect.x) room[inserted] = { x, y: rect.y };

    const nextNode = byId.get(next);
    const shift = nextNode ? x + rect.width + COLUMN_GAP - nextNode.position.x : 0;
    if (!nextNode || shift <= 0) return room;
    // next and what it feeds as data flows, only through the nodes right of where the inserted one was dropped
    const right = (node: Node) => !isFrame(node) && node.id !== inserted && nodeRect(node).x + nodeRect(node).width / 2 > middle;
    const moved = new Set<string>();
    const stack = right(nextNode) ? [next] : [];
    while (stack.length > 0) {
        const id = stack.pop()!;
        if (moved.has(id)) continue;
        moved.add(id);
        for (const edge of edges.filter((e) => e.target === id)) {
            const node = byId.get(edge.source);
            if (node && right(node) && !moved.has(node.id)) stack.push(node.id);
        }
    }
    for (const id of moved) room[id] = { x: byId.get(id)!.position.x + shift, y: byId.get(id)!.position.y };
    return room;
}

// how far along the glide is at `t` (0 to 1) of the way through: cubic-bezier(0.4, 0, 0.2, 1), the curve the simplified
// view's nodes glide on (index.css)
export function glideEase(t: number): number {
    const bezier = (a: number, b: number, u: number) => 3 * a * u * (1 - u) * (1 - u) + 3 * b * u * u * (1 - u) + u * u * u;
    // the curve's u for time t, it only goes up
    let [lo, hi] = [0, 1];
    for (let i = 0; i < 20; i++) {
        const u = (lo + hi) / 2;
        if (bezier(0.4, 0.2, u) < t) lo = u;
        else hi = u;
    }
    return bezier(0, 1, (lo + hi) / 2);
}
