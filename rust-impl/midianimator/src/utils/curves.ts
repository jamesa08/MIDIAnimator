// the curves the graph window draws, from graph_curves (src-tauri/src/graph/curves.rs). times are seconds

export type Point = [number, number];

// one piece of a curve's drawing, from where the last one ended (the first starts at the first key)
export type Segment = { kind: "bezier"; c1: Point; c2: Point; to: Point } | { kind: "line"; to: Point } | { kind: "step"; to: Point } | { kind: "points"; points: Point[] };

// `handles`: each key's left and right handle, null on a side that isn't a bezier
export type Drawing = { keys: Point[]; handles: [Point | null, Point | null][]; segments: Segment[]; slope_before: number; slope_after: number };

// a curve in the list. `extend`: drawn on past its ends. `pieces`: one curve, or one curve's keys from separate notes
export type CurveChannel = { id: string; group: string; name: string; axis: string | null; extend: boolean; pieces: Drawing[] };

export type NodeCurves = { node: string; channels: CurveChannel[] };

// a channel with its color, `key` is unique across the nodes
export type ShownChannel = CurveChannel & { key: string; color: string };

// x, y and z like Blender's axes, w for quaternions
const AXIS_COLORS: Record<string, string> = { X: "#e5484d", Y: "#3fa34d", Z: "#3b7ddd", W: "#c9a227" };
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
