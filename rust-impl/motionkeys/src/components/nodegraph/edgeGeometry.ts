// where the drawn edges go, in flow coordinates: what box select takes (NodeGraphCanvas)

export type Box = { x0: number; y0: number; x1: number; y1: number };
type XY = { x: number; y: number };

// every drawn edge's path by its id, looked up once per box move instead of once per edge
export function edgePaths(root: Element | null | undefined): Map<string, SVGPathElement> {
    const paths = new Map<string, SVGPathElement>();
    root?.querySelectorAll<SVGPathElement>(".react-flow__edge .react-flow__edge-path").forEach((path) => {
        const id = path.closest(".react-flow__edge")?.getAttribute("data-id");
        if (id) paths.set(id, path);
    });
    return paths;
}

// a bezier edge's path (getBezierPath's "M x,y C x,y x,y x,y") as its 8 numbers, null for any other shape
const CUBIC = /^\s*M[\d\s.,eE+-]*C[\d\s.,eE+-]*$/;
function cubicOf(d: string | null): number[] | null {
    if (!d || !CUBIC.test(d)) return null;
    const numbers = d.match(/-?[\d.]+(?:e[+-]?\d+)?/gi)?.map(Number) ?? [];
    return numbers.length === 8 && numbers.every(Number.isFinite) ? numbers : null;
}

// points along a cubic bezier about every `step`, with plain math. the browser's getPointAtLength walks the path from
// its start on every call (WebKit's especially), a box over hundreds of long wires took seconds per mouse move
function cubicPoints([x0, y0, x1, y1, x2, y2, x3, y3]: number[], step: number): XY[] {
    const reach = Math.hypot(x1 - x0, y1 - y0) + Math.hypot(x2 - x1, y2 - y1) + Math.hypot(x3 - x2, y3 - y2);
    const steps = Math.max(1, Math.ceil(reach / step));
    const points: XY[] = [];
    for (let i = 0; i <= steps; i++) {
        const t = i / steps;
        const u = 1 - t;
        const [a, b, c, e] = [u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t];
        points.push({ x: a * x0 + b * x1 + c * x2 + e * x3, y: a * y0 + b * y1 + c * y2 + e * y3 });
    }
    return points;
}

// points along an edge's path about every `step`, a bezier's without asking the browser
function pathPoints(path: SVGPathElement, step: number): XY[] {
    const cubic = cubicOf(path.getAttribute("d"));
    if (cubic) return cubicPoints(cubic, step);
    const length = path.getTotalLength();
    const points: XY[] = [];
    for (let at = 0; at <= length; at += step) points.push(path.getPointAtLength(at));
    return points;
}

// whether an edge's path passes through a box (flow coordinates), sampled about every 4px along the path as it's drawn
export function edgeCrosses(path: SVGPathElement | undefined, box: Box): boolean {
    if (!path) return false;
    // nowhere near the box, no need to walk it. a bezier stays inside its control points
    const cubic = cubicOf(path.getAttribute("d"));
    if (cubic) {
        const [xs, ys] = [cubic.filter((_, i) => i % 2 === 0), cubic.filter((_, i) => i % 2 === 1)];
        if (Math.min(...xs) > box.x1 || Math.max(...xs) < box.x0 || Math.min(...ys) > box.y1 || Math.max(...ys) < box.y0) return false;
    } else {
        const bounds = path.getBBox();
        if (bounds.x > box.x1 || bounds.x + bounds.width < box.x0 || bounds.y > box.y1 || bounds.y + bounds.height < box.y0) return false;
    }
    return pathPoints(path, 4).some((p) => p.x >= box.x0 && p.x <= box.x1 && p.y >= box.y0 && p.y <= box.y1);
}
