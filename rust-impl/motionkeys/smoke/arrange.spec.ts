import { test, expect } from "@playwright/test";
import { ArrangeMode, ArrangeSocket, COLUMN_GAP, PART_GAP, ROW_GAP, arrange } from "../src/utils/arrange";
import { FRAME_PAD, frameRects, labelHeight, nodeRect } from "../src/utils/frames";
import { ZONE_PAD_Y } from "../src/utils/groups";

type Rect = { x: number; y: number; width: number; height: number };

// a node of `type` at x, y, 200 by 100 unless it's given a size
const node = (id: string, x = 0, y = 0, more: any = {}) => ({ id, type: more.type ?? "node", position: { x, y }, measured: { width: more.width ?? 200, height: more.height ?? 100 }, data: more.data ?? {}, ...(more.frame ? { frame: more.frame } : {}) });
const frame = (id: string, data: any = {}, more: any = {}) => ({ id, type: "frame", position: { x: more.x ?? 0, y: more.y ?? 0 }, width: more.width ?? 300, height: more.height ?? 200, data, ...(more.frame ? { frame: more.frame } : {}) });
// a link from `from`'s output to `to`'s input, stored the other way around
const link = (from: string, to: string, output = "out", input = "in", more: any = {}) => ({ id: `${from}.${output}>${to}.${input}`, source: to, sourceHandle: input, target: from, targetHandle: output, ...more });
// every node has an input and an output 30 down, or the sockets it's given
const sockets = (nodes: any[], own: Record<string, ArrangeSocket[]> = {}) =>
    Object.fromEntries(
        nodes.map((n) => [
            n.id,
            own[n.id] ?? [
                { side: "inputs", socket: "in", y: 30 },
                { side: "outputs", socket: "out", y: 30 },
            ],
        ])
    );

// the nodes moved where arrange put them
async function arranged(nodes: any[], edges: any[], ids: string[] = [], own: Record<string, ArrangeSocket[]> = {}, mode?: ArrangeMode) {
    const positions = await arrange(nodes, edges, ids, sockets(nodes, own), mode);
    const moved = nodes.map((n) => (positions[n.id] ? { ...n, position: positions[n.id] } : n));
    const rects = frameRects(moved);
    const rect = (id: string): Rect => rects.get(id) ?? nodeRect(moved.find((n) => n.id === id));
    return { positions, moved, rect };
}

const apart = (a: Rect, b: Rect) => a.x >= b.x + b.width || a.x + a.width <= b.x || a.y >= b.y + b.height || a.y + a.height <= b.y;
// no two of these overlap
function expectApart(rect: (id: string) => Rect, ids: string[]) {
    for (const a of ids) for (const b of ids) if (a < b) expect(apart(rect(a), rect(b)), `${a} and ${b} overlap`).toBe(true);
}

test("columns follow the links, sources on the left", async () => {
    const nodes = [node("out", 0, 0), node("b", 50, 400), node("a", 900, 30), node("src", 400, 200), node("late", 0, 900)];
    const edges = [link("src", "a"), link("a", "b"), link("b", "out"), link("src", "out", "out", "in2"), link("late", "out", "out", "in3")];
    const { positions, rect } = await arranged(nodes, edges, [], { out: ["in", "in2", "in3"].map((socket, i) => ({ side: "inputs", socket, y: 30 + 20 * i })) });
    // each a column right of its rightmost input, sources in the first column
    expect(positions.src.x).toBe(positions.late.x);
    expect(positions.a.x).toBe(positions.src.x + 200 + COLUMN_GAP);
    expect(positions.b.x).toBe(positions.a.x + 200 + COLUMN_GAP);
    expect(positions.out.x).toBe(positions.b.x + 200 + COLUMN_GAP);
    expectApart(rect, ["out", "b", "a", "src", "late"]);
    // the top left of what was there stays
    expect(Math.min(...Object.values(positions).map((p) => p.x))).toBe(0);
    expect(Math.min(...Object.values(positions).map((p) => p.y))).toBe(0);
});

test("a node's top output feeds toward the top", async () => {
    const nodes = [node("src"), node("low", 0, 0), node("high", 0, 500)];
    const edges = [link("src", "high", "top"), link("src", "low", "bottom")];
    const own: Record<string, ArrangeSocket[]> = {
        src: [
            { side: "outputs", socket: "top", y: 20 },
            { side: "outputs", socket: "bottom", y: 80 },
        ],
    };
    const { positions } = await arranged(nodes, edges, [], own);
    expect(positions.high.y).toBeLessThan(positions.low.y);
});

test("disconnected chains and lone nodes are stacked below each other", async () => {
    const nodes = [node("a1", 0, 0), node("a2", 0, 0), node("b1", 0, 50), node("b2", 0, 50), node("lone", 0, 100)];
    const edges = [link("a1", "a2"), link("b1", "b2")];
    const { positions, rect } = await arranged(nodes, edges);
    expectApart(rect, ["a1", "a2", "b1", "b2", "lone"]);
    // in the order they were, a gap apart
    expect(positions.b1.y).toBe(positions.a1.y + 100 + PART_GAP);
    expect(positions.lone.y).toBe(positions.b1.y + 100 + PART_GAP);
    expect(positions.lone.x).toBe(positions.a1.x);
});

test("tagged links count as links", async () => {
    const nodes = [node("b", 0, 0), node("a", 0, 0)];
    const { positions } = await arranged(nodes, [link("a", "b", "out", "in", { tagged: true })]);
    expect(positions.b.x).toBe(positions.a.x + 200 + COLUMN_GAP);
    expect(positions.b.y).toBe(positions.a.y);
});

test("a selection is arranged from its top left, nothing else moves", async () => {
    const nodes = [node("a", 500, 300), node("b", 450, 700), node("c", 0, 0), node("d", 2000, 0)];
    const edges = [link("a", "b"), link("b", "c"), link("c", "d")];
    const { positions } = await arranged(nodes, edges, ["a", "b"]);
    expect(Object.keys(positions).sort()).toEqual(["a", "b"]);
    expect(positions.a).toEqual({ x: 450, y: 300 });
    expect(positions.b).toEqual({ x: 450 + 200 + COLUMN_GAP, y: 300 });
});

test("a for each zone is one block, its input on the left and output on the right", async () => {
    const nodes = [node("src"), node("zin", 0, 0, { type: "for_each_input", data: { zone: "zout" } }), node("body1"), node("body2"), node("zout", 0, 0, { type: "for_each_output" }), node("after"), node("side", 0, 0)];
    // `side` is fed by the source and feeds what's after the zone, it's in a column the zone spans
    const edges = [link("src", "zin"), link("zin", "body1"), link("body1", "body2"), link("body2", "zout"), link("zout", "after"), link("src", "side"), link("side", "after", "out", "in2"), link("zin", "zout", "out", "in2")];
    const { positions, rect } = await arranged(nodes, edges);
    const members = ["zin", "body1", "body2", "zout"];
    expect(positions.zin.x).toBeLessThan(Math.min(positions.body1.x, positions.body2.x));
    expect(positions.zout.x).toBeGreaterThan(Math.max(positions.body1.x, positions.body2.x));
    // the zone's frame as ZoneFrames draws it, nothing else is in it
    const rects = members.map(rect);
    const box = { x: Math.min(...rects.map((r) => r.x)), y: Math.min(...rects.map((r) => r.y)) - ZONE_PAD_Y, width: 0, height: 0 };
    box.width = Math.max(...rects.map((r) => r.x + r.width)) - box.x;
    box.height = Math.max(...rects.map((r) => r.y + r.height)) + ZONE_PAD_Y - box.y;
    for (const id of ["src", "after", "side"]) expect(apart(rect(id), box), `${id} is in the zone`).toBe(true);
    expectApart(rect, [...members, "src", "after", "side"]);
});

test("a frame's nodes stay in it as one block", async () => {
    const nodes = [node("src", 0, 0), frame("f"), node("a", 0, 0, { frame: "f" }), node("b", 0, 0, { frame: "f" }), node("out", 0, 0), node("other", 0, 0)];
    const edges = [link("src", "a"), link("a", "b"), link("b", "out"), link("src", "other"), link("other", "out", "out", "in2")];
    const { positions, rect } = await arranged(nodes, edges);
    // fitted around its nodes, nothing else in it
    const f = rect("f");
    expect(f.x).toBe(Math.min(positions.a.x, positions.b.x) - FRAME_PAD);
    expect(f.y).toBe(Math.min(positions.a.y, positions.b.y) - FRAME_PAD - labelHeight(nodes[1]));
    for (const id of ["src", "out", "other"]) expect(apart(rect(id), f), `${id} is in the frame`).toBe(true);
    expectApart(rect, ["src", "a", "b", "out", "other"]);
});

test("a frame with shrink off keeps its size, its nodes from its top left", async () => {
    const nodes = [frame("f", { shrink: false }, { width: 900, height: 600 }), node("a", 0, 0, { frame: "f" }), node("b", 0, 0, { frame: "f" }), node("out")];
    const edges = [link("a", "b"), link("b", "out")];
    const { positions, rect } = await arranged(nodes, edges);
    expect(positions.f).toBeDefined();
    expect(rect("f").width).toBe(900);
    expect(rect("f").height).toBe(600);
    expect(positions.a).toEqual({ x: positions.f.x + FRAME_PAD, y: positions.f.y + FRAME_PAD + labelHeight(nodes[0]) });
    // what's after it goes past all of it
    expect(positions.out.x).toBeGreaterThanOrEqual(positions.f.x + 900 + COLUMN_GAP);
});

test("a locked frame and its nodes stay, nothing lands on it", async () => {
    const nodes = [frame("lock", { locked: true }), node("in_lock", 100, 100, { frame: "lock" }), node("a", 0, 0), node("b", 0, 0), node("c", 0, 0)];
    const edges = [link("a", "b"), link("b", "c"), link("in_lock", "a")];
    const { positions, rect } = await arranged(nodes, edges);
    expect(positions.lock).toBeUndefined();
    expect(positions.in_lock).toBeUndefined();
    for (const id of ["a", "b", "c"]) expect(apart(rect(id), rect("lock")), `${id} is on the locked frame`).toBe(true);
});

test("a group's input is on the far left and its output on the far right", async () => {
    const nodes = [node("gout", 0, 0, { type: "group_output" }), node("gin", 0, 0, { type: "group_input" }), node("a"), node("b"), node("const")];
    const edges = [link("gin", "a"), link("a", "b"), link("b", "gout"), link("const", "gout", "out", "in2"), link("gin", "gout", "out", "in3")];
    const { positions } = await arranged(nodes, edges);
    const xs = Object.entries(positions)
        .filter(([id]) => id !== "gin" && id !== "gout")
        .map(([, p]) => p.x);
    expect(positions.gin.x).toBeLessThanOrEqual(Math.min(...xs));
    expect(positions.gout.x).toBeGreaterThan(Math.max(...xs));
});

test("collapsed nodes are spaced by the size they're drawn at", async () => {
    const nodes = [node("src"), node("a", 0, 0, { height: 28, data: { collapsed: true } }), node("b", 0, 0, { height: 28, data: { collapsed: true } }), node("c", 0, 0, { height: 28, data: { collapsed: true } })];
    const edges = [link("src", "a"), link("src", "b"), link("src", "c")];
    const own = Object.fromEntries(["a", "b", "c"].map((id) => [id, [{ side: "inputs" as const, socket: "in", y: 14 }]]));
    const { positions, rect } = await arranged(nodes, edges, [], own);
    expectApart(rect, ["src", "a", "b", "c"]);
    const ys = ["a", "b", "c"].map((id) => positions[id].y).sort((p, q) => p - q);
    // a column of headers is as tall as they are with the gaps between them
    expect(ys[2] - ys[0]).toBeLessThan(2 * 100);
});

const MODES: ArrangeMode[] = ["network_simplex", "brandes_koepf", "top_columns", "top_links"];

// sockets at different heights on each end, so lining up sockets and lining up tops differ
const offset: Record<string, ArrangeSocket[]> = {
    a: [{ side: "outputs", socket: "out", y: 30 }],
    b: [
        { side: "inputs", socket: "in", y: 60 },
        { side: "outputs", socket: "out", y: 30 },
    ],
    c: [{ side: "inputs", socket: "in", y: 60 }],
};

for (const mode of MODES) {
    test(`${mode}: zones, frames and parts keep apart, columns follow the links`, async () => {
        const nodes = [node("src"), node("zin", 0, 0, { type: "for_each_input", data: { zone: "zout" } }), node("body"), node("zout", 0, 0, { type: "for_each_output" }), frame("f"), node("fa", 0, 0, { frame: "f" }), node("fb", 0, 0, { frame: "f", height: 240 }), node("out"), node("lone1", 0, 900), node("lone2", 0, 900)];
        const edges = [link("src", "zin"), link("zin", "body"), link("body", "zout"), link("src", "fa"), link("fa", "fb"), link("zout", "out"), link("fb", "out", "out", "in2"), link("lone1", "lone2")];
        const { positions, rect } = await arranged(nodes, edges, [], {}, mode);
        const leaves = nodes.filter((n) => n.type !== "frame").map((n) => n.id);
        expectApart(rect, leaves);
        for (const e of edges) expect(positions[e.source].x, `${e.target} before ${e.source}`).toBeGreaterThan(positions[e.target].x);
        for (const id of ["src", "zin", "body", "zout", "out", "lone1", "lone2"]) expect(apart(rect(id), rect("f")), `${id} is in the frame`).toBe(true);
    });
}

test("network simplex and brandes-köpf line up the sockets of a link", async () => {
    for (const mode of ["network_simplex", "brandes_koepf"] as const) {
        const { positions } = await arranged([node("a"), node("b"), node("c")], [link("a", "b"), link("b", "c")], [], offset, mode);
        expect(positions.b.y + 60, mode).toBe(positions.a.y + 30);
        expect(positions.c.y + 60, mode).toBe(positions.b.y + 30);
    }
});

test("top-aligned links lines up the tops of linked nodes", async () => {
    const { positions } = await arranged([node("a"), node("b"), node("c")], [link("a", "b"), link("b", "c")], [], offset, "top_links");
    expect(positions.b.y).toBe(positions.a.y);
    expect(positions.c.y).toBe(positions.a.y);
});

test("top-aligned columns stacks every column from the same top", async () => {
    const nodes = [node("s1"), node("s2", 0, 0, { height: 60 }), node("m1", 0, 0, { height: 180 }), node("m2"), node("end")];
    const edges = [link("s1", "m1"), link("s2", "m2"), link("m1", "end"), link("m2", "end", "out", "in2")];
    const { positions } = await arranged(nodes, edges, [], {}, "top_columns");
    const columns = [["s1", "s2"], ["m1", "m2"], ["end"]].map((ids) => ids.map((id) => ({ id, ...positions[id], height: nodes.find((n) => n.id === id)!.measured.height })).sort((p, q) => p.y - q.y));
    for (const column of columns) {
        expect(column[0].y).toBe(columns[0][0].y);
        for (let i = 1; i < column.length; i++) expect(column[i].y).toBe(column[i - 1].y + column[i - 1].height + ROW_GAP);
    }
});
