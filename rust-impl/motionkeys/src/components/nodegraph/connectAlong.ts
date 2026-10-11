import { Edge, InternalNode } from "@xyflow/react";
import { SocketRef } from "../../utils/graphOps";
import { SocketPoint, sameSocket, socketPoints } from "./SocketHandle";

// how far (flow units) from the drop a node can be and still take sockets dragged along with a link
const ALONG_REACH = 600;

// one connection a socket dragged along makes: from an output to an input, and the socket it lands on
export type AlongLink = { socket: SocketPoint; target: SocketPoint; output: SocketRef; input: SocketRef };

// where the sockets dragged along with a link dropped from `output` to `input` connect, for the drop (the editor) and
// for drawing the drag (ConnectionLine). the ones below the dragged socket fill down from the drop, the rest of the
// dropped on node below it then the nearest nodes below, top to bottom. the ones above it fill up the same way, so no
// two links cross. each takes the next free socket whatever its type, like the dragged link. an input that's already
// connected or tagged is never taken, nor one that makes a cycle. outputs dropped on a multi input (`multi`) all go into
// it, top to bottom
export function connectAlong(nodeLookup: Map<string, InternalNode>, edges: Edge[], output: SocketRef, input: SocketRef, along: SocketPoint[], multi = false): AlongLink[] {
    if (along.length === 0) return [];
    const points = [...nodeLookup.values()].filter((n) => !n.hidden).flatMap(socketPoints);
    const at = (s: SocketRef) => points.find((p) => sameSocket(p, s));
    const side = along[0].side;
    const [from, drop] = side === "inputs" ? [at(input), at(output)] : [at(output), at(input)];
    if (!from || !drop) return [];

    // connections as data flows (stored edges are reversed), with the dropped one taking its input's place
    const key = (s: SocketRef) => `${s.node}\n${s.side}\n${s.socket}`;
    let links = edges.map((e) => ({ from: e.target, to: e.source, input: e.sourceHandle ?? "" }));
    const link = (out: SocketRef, inp: SocketRef) => {
        links = links.filter((l) => !(l.to === inp.node && l.input === inp.socket));
        links.push({ from: out.node, to: inp.node, input: inp.socket });
    };
    link(output, input);
    const reaches = (start: string, goal: string) => {
        const seen = new Set([start]);
        const stack = [start];
        while (stack.length > 0) {
            const node = stack.pop()!;
            if (node === goal) return true;
            for (const l of links.filter((l) => l.from === node && !seen.has(l.to))) {
                seen.add(l.to);
                stack.push(l.to);
            }
        }
        return false;
    };

    // a multi input takes every output that isn't already linked to it and doesn't make a cycle
    if (multi && side === "outputs") {
        const linked = (s: SocketRef) => edges.some((e) => e.source === input.node && e.sourceHandle === input.socket && e.target === s.node && e.targetHandle === s.socket);
        return along
            .filter((socket) => socket.node !== input.node && !linked(socket) && !reaches(input.node, socket.node))
            .sort((a, b) => a.y - b.y || a.x - b.x)
            .map((socket) => ({ socket, target: drop, output: socket, input: drop }));
    }
    const tagged = (s: SocketRef) => !!(nodeLookup.get(s.node)?.data as any)?.input_tags?.[s.socket];
    const taken = new Set([...links.map((l) => key({ node: l.to, side: "inputs", socket: l.input })), ...points.filter((p) => p.side === "inputs" && tagged(p)).map(key), key(output)]);

    // the sockets to fill going down from the drop and going up from it, in order
    const rows = (node: string) => points.filter((p) => p.node === node && p.side !== side).sort((a, b) => a.y - b.y);
    const gap = (node: InternalNode) => {
        const { x, y } = node.internals.positionAbsolute;
        const [width, height] = [node.measured.width ?? 0, node.measured.height ?? 0];
        return Math.hypot(Math.max(x - drop.x, 0, drop.x - x - width), Math.max(y - drop.y, 0, drop.y - y - height));
    };
    const middle = (node: InternalNode) => node.internals.positionAbsolute.y + (node.measured.height ?? 0) / 2;
    const others = [...nodeLookup.values()].filter((n) => !n.hidden && n.id !== drop.node && gap(n) <= ALONG_REACH).sort((a, b) => gap(a) - gap(b));
    const own = rows(drop.node);
    const down = [...own.filter((p) => p.y > drop.y), ...others.filter((n) => middle(n) > drop.y).flatMap((n) => rows(n.id))];
    const up = [...own.filter((p) => p.y < drop.y).reverse(), ...others.filter((n) => middle(n) <= drop.y).flatMap((n) => rows(n.id).reverse())];

    const found: AlongLink[] = [];
    const fill = (sockets: SocketPoint[], slots: SocketPoint[]) => {
        for (const socket of sockets) {
            const point = slots.find((point) => {
                if (taken.has(key(point))) return false;
                const [out, inp] = side === "outputs" ? [socket, point] : [point, socket];
                return out.node !== inp.node && !reaches(inp.node, out.node);
            });
            if (!point) continue;
            const [out, inp] = side === "outputs" ? [socket, point] : [point, socket];
            link(out, inp);
            taken.add(key(point));
            found.push({ socket, target: point, output: out, input: inp });
        }
    };
    const below = (socket: SocketPoint) => socket.y > from.y || (socket.y === from.y && socket.x > from.x);
    fill(
        along.filter(below).sort((a, b) => a.y - b.y || a.x - b.x),
        down
    );
    fill(
        along.filter((s) => !below(s)).sort((a, b) => b.y - a.y || b.x - a.x),
        up
    );
    return found;
}
