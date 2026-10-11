// the curves the graph window draws, from graph_curves (src-tauri/src/graph/curves.rs). times are seconds

export type Point = [number, number];

// one piece of a curve's drawing, from where the last one ended (the first starts at the first key)
export type Segment = { kind: "bezier"; c1: Point; c2: Point; to: Point } | { kind: "line"; to: Point } | { kind: "step"; to: Point } | { kind: "points"; points: Point[] };

// `handles`: each key's left and right handle, null on a side that isn't a bezier
export type Drawing = { keys: Point[]; handles: [Point | null, Point | null][]; segments: Segment[]; slope_before: number; slope_after: number };

// a curve in the list. `data_path` and `array_index`: the property it animates. `extend`: drawn on past its ends.
// `pieces`: one curve, or one curve's keys from separate notes
export type CurveChannel = { id: string; group: string; name: string; axis: string | null; data_path: string; array_index: number; extend: boolean; pieces: Drawing[] };

export type NodeCurves = { node: string; channels: CurveChannel[] };

// a channel with its color, `key` is unique across the nodes
export type ShownChannel = CurveChannel & { key: string; color: string };

// x, y and z like Blender's axes, w for quaternions
export const AXIS_COLORS: Record<string, string> = { X: "#e5484d", Y: "#3fa34d", Z: "#3b7ddd", W: "#c9a227" };
// everything that isn't an axis, in turn
const COLORS = ["#d6409f", "#8e4ec6", "#12a594", "#f76b15", "#a18072", "#0090ff", "#978365"];

// every channel of every node with its color
export function colorChannels(nodes: NodeCurves[]): ShownChannel[] {
    let other = 0;
    return nodes.flatMap((node) =>
        node.channels.map((channel) => ({
            ...channel,
            key: `${node.node}\n${channel.id}`,
            color: (channel.axis && AXIS_COLORS[channel.axis]) || COLORS[other++ % COLORS.length],
        }))
    );
}

export const segmentEnd = (segment: Segment): Point => (segment.kind === "points" ? segment.points[segment.points.length - 1] : segment.to);

// the time and value range of the channels' keys and everything drawn between them, null when there's nothing
export function curveBounds(channels: CurveChannel[]): { x0: number; x1: number; y0: number; y1: number } | null {
    let x0 = Infinity;
    let x1 = -Infinity;
    let y0 = Infinity;
    let y1 = -Infinity;
    const add = ([x, y]: Point) => {
        x0 = Math.min(x0, x);
        x1 = Math.max(x1, x);
        y0 = Math.min(y0, y);
        y1 = Math.max(y1, y);
    };
    for (const channel of channels) {
        for (const piece of channel.pieces) {
            piece.keys.forEach(add);
            for (const segment of piece.segments) {
                if (segment.kind === "bezier") {
                    // a bezier stays between its control points
                    add(segment.c1);
                    add(segment.c2);
                } else if (segment.kind === "points") {
                    segment.points.forEach(add);
                }
            }
        }
    }
    return x0 <= x1 ? { x0, x1, y0, y1 } : null;
}

// MARK: - Evaluating

// a bezier's value at time `t`, the time it's at found by halving (a curve's time only goes forward)
function bezierAt(from: Point, segment: { c1: Point; c2: Point; to: Point }, t: number): number {
    const at = (u: number, a: number, b: number, c: number, d: number) => {
        const v = 1 - u;
        return v * v * v * a + 3 * v * v * u * b + 3 * v * u * u * c + u * u * u * d;
    };
    let lo = 0;
    let hi = 1;
    for (let i = 0; i < 30; i++) {
        const middle = (lo + hi) / 2;
        if (at(middle, from[0], segment.c1[0], segment.c2[0], segment.to[0]) < t) lo = middle;
        else hi = middle;
    }
    const u = (lo + hi) / 2;
    return at(u, from[1], segment.c1[1], segment.c2[1], segment.to[1]);
}

// a straight line's value at time `t`
function lineAt(from: Point, to: Point, t: number): number {
    return to[0] - from[0] < 1e-12 ? to[1] : from[1] + ((t - from[0]) / (to[0] - from[0])) * (to[1] - from[1]);
}

// one curve's value at time `t`, carried on past its ends with its slopes when it's `extend`ed and held when it's not
function drawingAt(piece: Drawing, t: number, extend: boolean): number {
    const first = piece.keys[0];
    const last = piece.keys[piece.keys.length - 1];
    if (t <= first[0]) return extend ? first[1] - piece.slope_before * (first[0] - t) : first[1];
    if (t >= last[0]) return extend ? last[1] + piece.slope_after * (t - last[0]) : last[1];

    // the segment the time is in, from where the last one ended
    let from = first;
    for (const segment of piece.segments) {
        const to = segmentEnd(segment);
        if (t <= to[0]) {
            if (segment.kind === "bezier") return bezierAt(from, segment, t);
            if (segment.kind === "line") return lineAt(from, to, t);
            // constant holds until the next key
            if (segment.kind === "step") return t < to[0] ? from[1] : to[1];
            let previous = from;
            for (const point of segment.points) {
                if (t <= point[0]) return lineAt(previous, point, t);
                previous = point;
            }
            return to[1];
        }
        from = to;
    }
    return last[1];
}

// a channel's value at time `t` in seconds, null when it has no keys. a channel of separate notes' keys takes the last
// note that's started by then, the first one before any have
export function channelValue(channel: CurveChannel, t: number): number | null {
    const pieces = channel.pieces.filter((piece) => piece.keys.length > 0);
    if (pieces.length === 0) return null;
    const start = (piece: Drawing) => piece.keys[0][0];
    const started = pieces.filter((piece) => start(piece) <= t);
    const piece = started.length > 0 ? started.reduce((a, b) => (start(b) >= start(a) ? b : a)) : pieces.reduce((a, b) => (start(b) < start(a) ? b : a));
    return drawingAt(piece, t, channel.extend);
}
