import { Edge, InternalNode } from "@xyflow/react";
import { Op } from "../../utils/graphOps";
import { isFrame } from "../../utils/frames";
import { GroupDef, SpecLookup, inputHandle, outputHandle } from "../../utils/groups";
import { linkRank, wouldCycle } from "../../utils/sockets";
import { socketPoints } from "./SocketHandle";

// blender's delete with reconnect: nodes are deleted and what went through them is wired around them. each link out of
// a deleted node is taken over by what fed the node's input that passes through to that output: the type closest to the
// output's (linkRank), only one the output reads, the top one on a tie. it has to fit the input the link went into, nor
// make a cycle. a chain of deleted nodes is bridged end to end

// a wire as data flows, from an output to an input
type Link = { from: string; output: string; to: string; input: string };
const key = (l: Link) => `${l.from}\n${l.output}\n${l.to}\n${l.input}`;

// the ops that delete `deleted` and wire around them. `reads` gives the inputs a partial output reads (null for all).
// stored edges are reversed: `target` gives the value, `source` takes it. tags aren't wires, they're left as they are
export function deleteReconnect(lookup: SpecLookup, scope: GroupDef | null, nodeLookup: Map<string, InternalNode>, edges: Edge[], deleted: string[], reads: (node: string, output: string) => string[] | null): Op[] {
    const wires = edges.filter((e) => !(e as any).tagged);
    const before = wires.map((e) => ({ from: e.target, output: e.targetHandle ?? "", to: e.source, input: e.sourceHandle ?? "" }));
    const type = (node: string, side: "inputs" | "outputs", socket: string) => {
        const at = nodeLookup.get(node);
        return at ? (side === "inputs" ? inputHandle : outputHandle)(lookup, at, socket, scope).data_type : "Any";
    };

    // one node at a time, so a link bridged around one is there to bridge around the next. links keep their order, the
    // one taking over a link goes where it was (a multi input's order is what it gets)
    let links = before;
    for (const id of deleted) {
        const node = nodeLookup.get(id);
        const order = node
            ? socketPoints(node)
                  .filter((p) => p.side === "inputs")
                  .sort((a, b) => a.y - b.y)
                  .map((p) => p.socket)
            : [];
        const into = links.filter((l) => l.to === id).sort((a, b) => order.indexOf(a.input) - order.indexOf(b.input));
        const kept = links.filter((l) => l.from !== id && l.to !== id);
        const bridges = new Map<Link, Link>();
        for (const out of links.filter((l) => l.from === id)) {
            if (!node || isFrame(node)) break;
            const passes = reads(id, out.output);
            const toType = type(out.to, "inputs", out.input);
            const outType = type(id, "outputs", out.output);
            let best: Link | null = null;
            let bestRank = Infinity;
            for (const link of into) {
                if (passes && !passes.includes(link.input)) continue;
                if (linkRank(type(link.from, "outputs", link.output), toType) === Infinity) continue;
                const rank = linkRank(type(id, "inputs", link.input), outType);
                if (rank < bestRank) [best, bestRank] = [link, rank];
            }
            if (!best) continue;
            const bridge = { from: best.from, output: best.output, to: out.to, input: out.input };
            const current = [...kept, ...bridges.values()];
            if (current.some((l) => key(l) === key(bridge)) || wouldCycle(current, reads, bridge.from, bridge.output, bridge.to, bridge.input)) continue;
            bridges.set(out, bridge);
        }
        links = links.flatMap((l) => (bridges.has(l) ? [bridges.get(l)!] : l.from === id || l.to === id ? [] : [l]));
    }

    // a multi input that takes a new link is wired again in order, the others just take theirs
    const had = new Set(before.map(key));
    const added = links.filter((l) => !had.has(key(l)));
    const ops: Op[] = [];
    const remade: string[] = [];
    const done = new Set<string>();
    for (const link of added) {
        const input = `${link.to}\n${link.input}`;
        if (done.has(input)) continue;
        done.add(input);
        const node = nodeLookup.get(link.to);
        const multi = !!node && !!inputHandle(lookup, node, link.input, scope).multi;
        const into = multi ? links.filter((l) => l.to === link.to && l.input === link.input) : [link];
        if (multi) remade.push(...wires.filter((e) => e.source === link.to && e.sourceHandle === link.input && !deleted.includes(e.target)).map((e) => e.id));
        for (const l of into) ops.push({ op: "connect", from_node: l.from, from_output: l.output, to_node: l.to, to_input: l.input });
    }
    return [{ op: "delete", nodes: deleted, ...(remade.length > 0 ? { edges: remade } : {}) }, ...ops];
}
