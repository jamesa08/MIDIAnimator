// frames: labeled boxes around nodes (src-tauri/src/graph/frames.rs). a node in one names it in its `frame` field (a
// frame in a frame the same way) and positions stay absolute, so moving a frame moves each node in it. a frame is drawn
// fitted around what's in it by the sizes on screen. a locked one stays behind the graph: it can't be moved, resized or
// box selected, and only a click on its label selects it

type XY = { x: number; y: number };
export type Rect = { x: number; y: number; width: number; height: number };

export const FRAME = "frame";
// room around what's in a frame, the label's size when none is set and the room above and below it in its strip, keep
// in sync with frames.rs
export const FRAME_PAD = 20;
export const FRAME_LABEL_SIZE = 20;
const LABEL_PAD = 16;
// a frame's color when it has none, keep in sync with frames.rs
export const FRAME_COLOR = "#999999";
// frames are drawn under the edges and nodes, a frame in a frame above the one it's in. react flow lifts selected nodes
// by 1000, a selected frame is put back down by as much
const FRAME_Z = -2000;
const SELECTED_Z = 1000;
// a node's size before it's measured, like frames.rs
const NODE_WIDTH = 200;
const NODE_HEIGHT = 100;
// how far in from a frame's sides a box select has to start and stay to not take the frame, like blender's border
const BORDER = 10;

export const isFrame = (node: any) => node?.type === FRAME;

// whether a frame fits tight around what's in it, on unless it was given a size (resized). one being resized is drawn
// at the size it's given
export const shrinks = (frame: any) => frame.data?.shrink !== false && !frame.resizing;
// whether a frame keeps exactly its size, what's in it can stick out
export const locksSize = (frame: any) => frame.data?.lock_size === true;
export const isLocked = (frame: any) => isFrame(frame) && frame.data?.locked === true;
// the label's size, and how tall the strip at the top with it is
export const labelSize = (frame: any): number => frame.data?.label_size ?? FRAME_LABEL_SIZE;
export const labelHeight = (frame: any) => labelSize(frame) + LABEL_PAD;

// the frames a node is in, innermost first
export function frameAncestors(byId: Map<string, any>, id: string): string[] {
    const out: string[] = [];
    let at: string | undefined = byId.get(id)?.frame;
    while (at && at !== id && !out.includes(at)) {
        out.push(at);
        at = byId.get(at)?.frame;
    }
    return out;
}

// the ids and everything inside the frames among them, at any depth
export function withContents(nodes: any[], ids: string[]): string[] {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const picked = new Set(ids);
    return nodes.filter((n) => picked.has(n.id) || frameAncestors(byId, n.id).some((f) => picked.has(f))).map((n) => n.id);
}

// where a node is and how big, as it's drawn
export function nodeRect(node: any): Rect {
    const [width, height] = isFrame(node) ? [node.width, node.height] : [node.measured?.width ?? node.width, node.measured?.height ?? node.height];
    return { x: node.position.x, y: node.position.y, width: width ?? NODE_WIDTH, height: height ?? NODE_HEIGHT };
}

const contains = (r: Rect, p: XY) => p.x >= r.x && p.x <= r.x + r.width && p.y >= r.y && p.y <= r.y + r.height;
export const apart = (a: Rect, b: Rect) => a.x >= b.x + b.width || a.x + a.width <= b.x || a.y >= b.y + b.height || a.y + a.height <= b.y;
const center = (r: Rect) => ({ x: r.x + r.width / 2, y: r.y + r.height / 2 });

// the box around rects
export function bounds(rects: Rect[]): Rect {
    const x0 = Math.min(...rects.map((r) => r.x));
    const y0 = Math.min(...rects.map((r) => r.y));
    const x1 = Math.max(...rects.map((r) => r.x + r.width));
    const y1 = Math.max(...rects.map((r) => r.y + r.height));
    return { x: x0, y: y0, width: x1 - x0, height: y1 - y0 };
}

// where each frame is drawn, innermost first: around what's directly in it like a for each zone, with room for its
// label. a frame given a size keeps at least that and grows past it to take in its nodes, an empty one or one with its
// size locked stays as it is
export function frameRects(nodes: any[]): Map<string, Rect> {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const depth = new Map(nodes.filter(isFrame).map((f) => [f.id, frameAncestors(byId, f.id).length]));
    const rects = new Map<string, Rect>();
    for (const frame of nodes.filter(isFrame).sort((a, b) => depth.get(b.id)! - depth.get(a.id)!)) {
        const members = nodes.filter((n) => n.frame === frame.id).map((n) => (isFrame(n) ? rects.get(n.id)! : nodeRect(n)));
        if (members.length === 0 || locksSize(frame)) {
            rects.set(frame.id, nodeRect(frame));
            continue;
        }
        const inside = bounds(members);
        const label = labelHeight(frame);
        const fit = { x: inside.x - FRAME_PAD, y: inside.y - FRAME_PAD - label, width: inside.width + 2 * FRAME_PAD, height: inside.height + 2 * FRAME_PAD + label };
        rects.set(frame.id, shrinks(frame) ? fit : bounds([fit, nodeRect(frame)]));
    }
    return rects;
}

// the nodes as react flow draws them: each frame where it's drawn (fitted to what's in it unless `fit` is false, the
// frozen graph behind an open group draws the backend's fit), under everything, moved by its label unless it's locked
// and selected by the editor
export function layoutFrames(nodes: any[], fit: boolean): any[] {
    if (!nodes.some(isFrame)) return nodes;
    const rects = fit ? frameRects(nodes) : null;
    const byId = new Map(nodes.map((n) => [n.id, n]));
    return nodes.map((node) => {
        if (!isFrame(node)) return node;
        const rect = rects?.get(node.id) ?? nodeRect(node);
        const z = FRAME_Z + frameAncestors(byId, node.id).length - (node.selected ? SELECTED_Z : 0);
        const locked = isLocked(node);
        return { ...node, position: { x: rect.x, y: rect.y }, width: rect.width, height: rect.height, zIndex: z, selectable: false, draggable: !locked, className: locked ? "frame-locked" : undefined, dragHandle: ".graph-frame-label", style: { ...node.style, pointerEvents: "none" } };
    });
}

// the innermost of `frames` (as drawn, by id) a point is in
function innermost(byId: Map<string, any>, frames: [string, Rect][], at: (rect: Rect) => boolean): string | null {
    const depth = (id: string) => frameAncestors(byId, id).length;
    return frames.filter(([, rect]) => at(rect)).sort(([a], [b]) => depth(b) - depth(a))[0]?.[0] ?? null;
}

// the frames as drawn (layoutFrames), by id
export const drawnFrames = (nodes: any[]) => new Map<string, Rect>(nodes.filter(isFrame).map((f) => [f.id, nodeRect(f)]));

// the innermost frame drawn under `point` (nodes as drawn), only the unlocked ones with `unlocked`
export function frameAt(nodes: any[], point: XY, unlocked = false): string | null {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const frames = [...drawnFrames(nodes)].filter(([id]) => !(unlocked && isLocked(byId.get(id))));
    return innermost(byId, frames, (rect) => contains(rect, point));
}

// whether a box select takes a frame (as drawn), like blender: the box touches it but isn't wholly inside it, away from
// its label and sides. never a locked one
export function boxTakesFrame(frame: any, rect: Rect, box: Rect): boolean {
    if (isLocked(frame)) return false;
    const label = labelHeight(frame);
    const inner = { x: rect.x + BORDER, y: rect.y + label, width: rect.width - 2 * BORDER, height: rect.height - label - BORDER };
    const within = box.x >= inner.x && box.y >= inner.y && box.x + box.width <= inner.x + inner.width && box.y + box.height <= inner.y + inner.height;
    return !apart(rect, box) && !within;
}

// the frame each moved node joins after a drop, only the ones that change. `frames` are the frames as drawn when the
// move started. like a for each zone, a node in a frame stays in it however far it's dragged (the frame grows to take
// it in), remove from frame takes it out. a node that lands in a frame (its middle or the cursor over it) joins the
// innermost one, if it's in no frame or in one around that one. what a moved frame carries stays in it
export function dropFrames(nodes: any[], moved: string[], frames: Map<string, Rect>, cursor: XY): Record<string, string> {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const movedSet = new Set(moved);
    const roots = moved.filter((id) => byId.has(id) && !frameAncestors(byId, id).some((f) => movedSet.has(f)));
    const carried = new Set(withContents(nodes, roots));
    const targets = [...frames].filter(([id]) => byId.has(id) && !carried.has(id));

    const changes: Record<string, string> = {};
    for (const id of roots) {
        const node = byId.get(id);
        const rect = nodeRect(node);
        const landed = innermost(byId, targets, (r) => contains(r, center(rect)) || contains(r, cursor));
        if (landed && landed !== node.frame && (!node.frame || frameAncestors(byId, landed).includes(node.frame))) changes[id] = landed;
    }
    return changes;
}
