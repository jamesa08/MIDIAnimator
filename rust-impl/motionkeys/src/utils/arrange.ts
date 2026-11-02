import type { ElkExtendedEdge, ElkNode, ElkPort, LayoutOptions } from "elkjs/lib/elk-api";
import { FRAME_PAD, Rect, apart, bounds, frameAncestors, frameRects, isFrame, isLocked, labelHeight, locksSize, nodeRect, shrinks, withContents } from "./frames";
import { GROUP_INPUT, GROUP_OUTPUT, ZONE_PAD_Y, findZones } from "./groups";

type XY = { x: number; y: number };

// one socket of a node and how far down from the node's top its middle is
export type ArrangeSocket = { side: "inputs" | "outputs"; socket: string; y: number };

// the gap between columns, between nodes in a column, and between disconnected parts (and a locked frame) stacked
// below each other
export const COLUMN_GAP = 80;
export const ROW_GAP = 30;
export const PART_GAP = 80;

// how the nodes in each column are placed (the node_editor.arrange setting): lining up the sockets links go between
// (elk's network simplex or brandes-köpf), every column from the same top, or the tops of linked nodes lined up
export type ArrangeMode = "network_simplex" | "brandes_koepf" | "top_columns" | "top_links";

// the same for every block, elk doesn't carry them from a block into the blocks in it
const LAYOUT: LayoutOptions = {
    "elk.algorithm": "layered",
    "elk.direction": "RIGHT",
    "elk.layered.layering.strategy": "LONGEST_PATH_SOURCE",
    "elk.layered.spacing.nodeNodeBetweenLayers": String(COLUMN_GAP),
    "elk.spacing.nodeNode": String(ROW_GAP),
};
const PLACEMENT: Record<ArrangeMode, LayoutOptions> = {
    network_simplex: { "elk.layered.nodePlacement.strategy": "NETWORK_SIMPLEX" },
    brandes_koepf: { "elk.layered.nodePlacement.strategy": "BRANDES_KOEPF", "elk.layered.nodePlacement.bk.fixedAlignment": "BALANCED" },
    top_columns: { "elk.layered.nodePlacement.strategy": "NETWORK_SIMPLEX" },
    top_links: { "elk.layered.nodePlacement.strategy": "NETWORK_SIMPLEX" },
};

const ROOT = "";
const ZONE = "zone\n";

// elk is big, it's loaded the first time something is arranged
let elkPromise: Promise<{ layout: (graph: ElkNode) => Promise<ElkNode> }> | null = null;
function loadElk() {
    elkPromise ??= import("elkjs/lib/elk.bundled.js").then(({ default: ELK }) => new ELK());
    return elkPromise;
}

const grow = (r: Rect, by: number) => ({ x: r.x - by, y: r.y - by, width: r.width + 2 * by, height: r.height + 2 * by });

// where each node goes (frames too) when the nodes `ids` are arranged, every node when there are none. the arranged
// nodes keep their top left. `nodes` are the graph's with the sizes they're drawn at (`measured`) and `sockets` where
// each node's sockets are, `edges` the stored ones (tagged ones count as links). only the nodes that move are in it
export async function arrange(nodes: any[], edges: any[], ids: string[], sockets: Record<string, ArrangeSocket[]>, mode: ArrangeMode = "network_simplex"): Promise<Record<string, XY>> {
    const layout = { ...LAYOUT, ...(PLACEMENT[mode] ?? PLACEMENT.network_simplex) };
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const drawn = frameRects(nodes);
    const rectOf = (id: string) => (isFrame(byId.get(id)) ? drawn.get(id)! : nodeRect(byId.get(id)));

    // a locked frame stays with what's in it, and so does every frame it's in
    const lockedFrames = nodes.filter(isLocked).flatMap((f) => [f.id, ...frameAncestors(byId, f.id)]);
    const fixed = new Set(withContents(nodes, lockedFrames));
    const picked = ids.length > 0 ? withContents(nodes, ids) : nodes.map((n) => n.id);
    const moving = new Set(picked.filter((id) => byId.has(id) && !fixed.has(id)));
    if (![...moving].some((id) => !isFrame(byId.get(id)))) return {};
    const obstacles = [...new Set(lockedFrames)].map(rectOf);

    // the blocks: a node in a frame that moves too is in its block, the rest are at the top
    const parent = new Map<string, string>([...moving].map((id) => [id, moving.has(byId.get(id).frame) ? byId.get(id).frame : ROOT]));
    const chain = (id: string) => {
        const out = [id];
        while (out[out.length - 1] !== ROOT) out.push(parent.get(out[out.length - 1])!);
        return out;
    };
    const kids = (id: string) => [...parent].filter(([, p]) => p === id).map(([c]) => c);
    const leavesUnder = (id: string): string[] => (byId.has(id) && !isFrame(byId.get(id)) ? [id] : kids(id).flatMap(leavesUnder));

    // a for each zone is a block of its own (the outer one of two first), between the frames around all of it and the
    // ones in it. a zone that's partly in a frame with other nodes, or partly not arranged, can't be one
    const zones = new Map<string, { input: string; output: string; members: string[] }>();
    for (const zone of findZones({ nodes, edges }).sort((a, b) => b.body.size - a.body.size)) {
        const members = [zone.input, zone.output, ...zone.body];
        if (!members.every((id) => moving.has(id))) continue;
        const chains = members.map(chain);
        const top = chains[0].find((p) => p !== members[0] && chains.every((c) => c.includes(p)))!;
        const under = [...new Set(chains.map((c) => c[c.indexOf(top) - 1]))];
        const inZone = new Set(members);
        if (!under.every((id) => leavesUnder(id).every((leaf) => inZone.has(leaf)))) continue;
        const id = ZONE + zone.input;
        zones.set(id, { input: zone.input, output: zone.output, members });
        parent.set(id, top);
        for (const c of under) parent.set(c, id);
    }
    const zoneOf = new Map([...zones.values()].flatMap((z) => [[z.input, "FIRST"] as const, [z.output, "LAST"] as const]));
    const outer = (id: string): Rect => (zones.has(id) ? zoneRect(zones.get(id)!.members.map(rectOf)) : rectOf(id));
    const order = (a: string, b: string) => outer(a).y - outer(b).y || outer(a).x - outer(b).x;

    // the links between nodes that move, from an output to an input
    const port = (node: string, side: string, socket: string) => `${node}\n${side}\n${socket}`;
    const links = edges.filter((e) => e.source !== e.target && moving.has(e.source) && moving.has(e.target) && !isFrame(byId.get(e.source)) && !isFrame(byId.get(e.target)));
    const used = new Map<string, Set<string>>();
    for (const e of links) {
        used.set(e.target, (used.get(e.target) ?? new Set()).add(`outputs\n${e.targetHandle}`));
        used.set(e.source, (used.get(e.source) ?? new Set()).add(`inputs\n${e.sourceHandle}`));
    }

    // a node with a port where each of its sockets is, inputs on the left and outputs on the right. lining up the tops
    // of linked nodes, they're all at the top in the same order
    const leaf = (id: string): ElkNode => {
        const node = byId.get(id);
        const { width, height } = nodeRect(node);
        const known = sockets[id] ?? [];
        const missing = [...(used.get(id) ?? [])].map((key) => key.split("\n") as ["inputs" | "outputs", string]).filter(([side, socket]) => !known.some((s) => s.side === side && s.socket === socket));
        const all = [...known, ...missing.map(([side, socket]) => ({ side, socket, y: height / 2 }))];
        const y = (s: ArrangeSocket) => (mode === "top_links" ? all.filter((o) => o.side === s.side && o.y < s.y).length * 0.01 : s.y);
        const ports: ElkPort[] = all.map((s) => ({ id: port(id, s.side, s.socket), x: s.side === "inputs" ? 0 : width, y: y(s), width: 0, height: 0, layoutOptions: { "elk.port.side": s.side === "inputs" ? "WEST" : "EAST" } }));
        // a group's input on the far left and its output on the far right, a zone's ends at the ends of the zone
        const layer = node.type === GROUP_INPUT ? "FIRST" : node.type === GROUP_OUTPUT ? "LAST" : zoneOf.get(id);
        return { id, width, height, ports, layoutOptions: { "elk.portConstraints": "FIXED_POS", ...(layer ? { "elk.layered.layering.layerConstraint": layer } : {}) } };
    };
    // a frame fits around its nodes with room for its label, one with Shrink off or its size locked stays at least as
    // big as it is. an empty one is like a node
    const block = (id: string): ElkNode => {
        const children = kids(id).sort(order).map(block);
        if (zones.has(id)) return { id, children, layoutOptions: { ...layout, "elk.padding": `[top=${ZONE_PAD_Y},left=0,bottom=${ZONE_PAD_Y},right=0]` } };
        const node = byId.get(id);
        if (!isFrame(node)) return leaf(id);
        const { width, height } = nodeRect(node);
        if (children.length === 0) return { id, width, height };
        const keep: LayoutOptions = !shrinks(node) || locksSize(node) ? { "elk.nodeSize.constraints": "MINIMUM_SIZE", "elk.nodeSize.minimum": `(${width}, ${height})` } : {};
        return { id, children, layoutOptions: { ...layout, ...keep, "elk.padding": `[top=${FRAME_PAD + labelHeight(node)},left=${FRAME_PAD},bottom=${FRAME_PAD},right=${FRAME_PAD}]` } };
    };

    // the parts that aren't linked to each other, top to bottom as they were
    const topOf = (id: string) => chain(id).at(-2)!;
    const part = new Map(kids(ROOT).map((id) => [id, id]));
    const find = (id: string): string => (part.get(id) === id ? id : find(part.get(id)!));
    for (const e of links) part.set(find(topOf(e.source)), find(topOf(e.target)));
    const parts = new Map<string, string[]>();
    for (const id of kids(ROOT).sort(order)) parts.set(find(id), [...(parts.get(find(id)) ?? []), id]);

    // each part laid out on its own, then stacked below the one before from the top left of what's arranged, and below
    // a locked frame it would land on
    const elk = await loadElk();
    const start = bounds([...moving].map(outer));
    const hit = (r: Rect) => obstacles.find((o) => !apart(r, grow(o, PART_GAP)));
    const positions: Record<string, XY> = {};
    let y = start.y;
    for (const items of parts.values()) {
        const inPart = new Set(items.flatMap(leavesUnder));
        const partLinks: ElkExtendedEdge[] = links.filter((e) => inPart.has(e.source)).map((e, i) => ({ id: `link${i}`, sources: [port(e.target, "outputs", e.targetHandle)], targets: [port(e.source, "inputs", e.sourceHandle)] }));
        const laid = await elk.layout({ id: "root", layoutOptions: { ...layout, "elk.hierarchyHandling": "INCLUDE_CHILDREN" }, children: items.map(block), edges: partLinks });

        // where everything is in the part, from its top left
        const found = new Map<string, Rect>();
        const walk = (node: ElkNode, x: number, y: number) => {
            for (const child of node.children ?? []) {
                const at = { x: x + (child.x ?? 0), y: y + (child.y ?? 0), width: child.width ?? 0, height: child.height ?? 0 };
                found.set(child.id, at);
                walk(child, at.x, at.y);
            }
        };
        walk(laid, 0, 0);
        if (mode === "top_columns") stackColumns(items, found, chain);
        const box = bounds(items.map((id) => found.get(id)!));
        let at = { x: start.x, y, width: box.width, height: box.height };
        for (let o = hit(at); o; o = hit(at)) at = { ...at, y: o.y + o.height + PART_GAP };
        for (const [id, r] of found) {
            if (!zones.has(id)) positions[id] = { x: Math.round(r.x - box.x + at.x), y: Math.round(r.y - box.y + at.y) };
        }
        y = at.y + at.height + PART_GAP;
    }
    return positions;
}

// every column of a part stacked from the part's top in the order elk put them in, with what's in each block moved
// along. elk's columns are apart, a column is the blocks whose sides overlap
function stackColumns(items: string[], found: Map<string, Rect>, chain: (id: string) => string[]) {
    const top = Math.min(...items.map((id) => found.get(id)!.y));
    const columns: string[][] = [];
    for (const id of [...items].sort((a, b) => found.get(a)!.x - found.get(b)!.x)) {
        const column = columns.at(-1);
        if (column && found.get(id)!.x < Math.max(...column.map((c) => found.get(c)!.x + found.get(c)!.width))) column.push(id);
        else columns.push([id]);
    }
    for (const column of columns) {
        let y = top;
        for (const id of column.sort((a, b) => found.get(a)!.y - found.get(b)!.y)) {
            const dy = y - found.get(id)!.y;
            y += found.get(id)!.height + ROW_GAP;
            for (const [key, r] of found) if (chain(key).includes(id)) found.set(key, { ...r, y: r.y + dy });
        }
    }
}

// the box a zone's frame is drawn in (ZoneFrames.tsx) as far as other nodes go: around its nodes, with room above and below
function zoneRect(members: Rect[]): Rect {
    const r = bounds(members);
    return { x: r.x, y: r.y - ZONE_PAD_Y, width: r.width, height: r.height + 2 * ZONE_PAD_Y };
}
