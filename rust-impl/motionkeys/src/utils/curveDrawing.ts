// drawing curves on a time and value grid on a canvas, shared by the graph window (components/graph/CurveCanvas.tsx) and
// the curve previews on nodes (components/graph/CurvePreview.tsx). times are seconds
import { ShownChannel, curveBounds, segmentEnd } from "./curves";

// the visible time (x) and value (y) range
export type View = { x0: number; x1: number; y0: number; y1: number };

// where curves are drawn: the canvas area from the top left, in css pixels, and the view it shows
export type Plot = { width: number; height: number; view: View };

// a css variable's value, the colors come from the theme
export type ThemeColor = (name: string) => string;

// grid spacing it aims for, in pixels
const GRID_X = 90;
const GRID_Y = 40;
// keys aren't drawn when more than this many are in view
const MAX_KEYS = 3000;

// a round grid step (1, 2 or 5 times a power of ten) near `raw`
export function gridStep(raw: number): number {
    const power = Math.pow(10, Math.floor(Math.log10(raw)));
    const n = raw / power;
    return (n < 1.5 ? 1 : n < 3.5 ? 2 : n < 7.5 ? 5 : 10) * power;
}

// a grid line's label with as many decimals as the step needs
export function gridLabel(value: number, step: number): string {
    const decimals = Math.max(0, -Math.floor(Math.log10(step)));
    // no "-0"
    return (Math.abs(value) < step / 2 ? 0 : value).toFixed(decimals);
}

// the view around the channels, with some room around them (a share of each axis' range). null when there's nothing
// to frame
export function framed(channels: ShownChannel[], padX = 0.04, padY = 0.08): View | null {
    const bounds = curveBounds(channels);
    if (!bounds) return null;
    let { x0, x1, y0, y1 } = bounds;
    if (x1 - x0 < 1e-9) {
        x0 -= 0.5;
        x1 += 0.5;
    }
    if (y1 - y0 < 1e-9) {
        y0 -= 1;
        y1 += 1;
    }
    const roomX = (x1 - x0) * padX;
    const roomY = (y1 - y0) * padY;
    return { x0: x0 - roomX, x1: x1 + roomX, y0: y0 - roomY, y1: y1 + roomY };
}

// time and value to pixels
export function toPixels({ width, height, view }: Plot) {
    const { x0, x1, y0, y1 } = view;
    return { sx: (x: number) => ((x - x0) / (x1 - x0)) * width, sy: (y: number) => height - ((y - y0) / (y1 - y0)) * height };
}

// the grid lines, the zero value a little stronger. `timeScale` turns seconds into the units shown (the fps for frames),
// time lines are round in those units and whole frames at the least. gives the lines for labels
export function drawGrid(ctx: CanvasRenderingContext2D, plot: Plot, timeScale: number, color: ThemeColor) {
    const { width, height, view } = plot;
    const { x0, x1, y0, y1 } = view;
    const { sx, sy } = toPixels(plot);
    const inFrames = timeScale !== 1;
    const rawStepX = gridStep(((x1 - x0) * timeScale * GRID_X) / Math.max(width, 1));
    const stepX = inFrames ? Math.max(1, rawStepX) : rawStepX;
    const stepY = gridStep(((y1 - y0) * GRID_Y) / Math.max(height, 1));
    const timeLines: number[] = [];
    for (let t = Math.ceil((x0 * timeScale) / stepX) * stepX; t <= x1 * timeScale; t += stepX) timeLines.push(t);
    const valueLines: number[] = [];
    for (let y = Math.ceil(y0 / stepY) * stepY; y <= y1; y += stepY) valueLines.push(y);

    ctx.lineWidth = 1;
    ctx.strokeStyle = color("--graph-grid");
    ctx.beginPath();
    for (const t of timeLines) {
        const px = Math.round(sx(t / timeScale)) + 0.5;
        ctx.moveTo(px, 0);
        ctx.lineTo(px, height);
    }
    for (const y of valueLines) {
        const py = Math.round(sy(y)) + 0.5;
        ctx.moveTo(0, py);
        ctx.lineTo(width, py);
    }
    ctx.stroke();
    if (y0 < 0 && y1 > 0) {
        ctx.strokeStyle = color("--graph-zero");
        ctx.beginPath();
        const py = Math.round(sy(0)) + 0.5;
        ctx.moveTo(0, py);
        ctx.lineTo(width, py);
        ctx.stroke();
    }
    return { timeLines, stepX, valueLines, stepY };
}

// the channels' curves, clipped to the plot. `keys` draws the keys on top, `handles` their handles too, both left out
// when there are too many in view
export function drawCurves(ctx: CanvasRenderingContext2D, plot: Plot, channels: ShownChannel[], options: { keys: boolean; handles: boolean; lineWidth?: number }, color: ThemeColor) {
    const { width, height, view } = plot;
    const { x0, x1, y0, y1 } = view;
    const { sx, sy } = toPixels(plot);

    ctx.save();
    ctx.beginPath();
    ctx.rect(0, 0, width, height);
    ctx.clip();
    ctx.lineWidth = options.lineWidth ?? 1.5;
    ctx.lineJoin = "round";
    // keys in view with their handles, in pixels
    type ShownKey = { x: number; y: number; color: string; left: [number, number] | null; right: [number, number] | null };
    const keys: ShownKey[] = [];
    const pixel = (point: [number, number] | null): [number, number] | null => (point ? [sx(point[0]), sy(point[1])] : null);
    for (const channel of channels) {
        ctx.strokeStyle = channel.color;
        ctx.beginPath();
        for (const piece of channel.pieces) {
            const first = piece.keys[0];
            const last = piece.keys[piece.keys.length - 1];
            if (!first) continue;
            if (!channel.extend && (last[0] < x0 || first[0] > x1)) continue;

            let [cx, cy] = first;
            if (channel.extend && cx > x0) {
                ctx.moveTo(0, sy(cy - piece.slope_before * (cx - x0)));
                ctx.lineTo(sx(cx), sy(cy));
            } else {
                ctx.moveTo(sx(cx), sy(cy));
            }
            for (const segment of piece.segments) {
                const [tx, ty] = segmentEnd(segment);
                if (cx > x1) break;
                if (tx < x0) {
                    // all of it is left of the view
                    ctx.moveTo(sx(tx), sy(ty));
                } else if (segment.kind === "step") {
                    ctx.lineTo(sx(tx), sy(cy));
                    ctx.lineTo(sx(tx), sy(ty));
                } else if (segment.kind === "line" || sx(tx) - sx(cx) < 1) {
                    // a segment narrower than a pixel is a line
                    ctx.lineTo(sx(tx), sy(ty));
                } else if (segment.kind === "bezier") {
                    ctx.bezierCurveTo(sx(segment.c1[0]), sy(segment.c1[1]), sx(segment.c2[0]), sy(segment.c2[1]), sx(tx), sy(ty));
                } else {
                    for (const [px, py] of segment.points) ctx.lineTo(sx(px), sy(py));
                }
                cx = tx;
                cy = ty;
            }
            if (channel.extend && cx < x1) ctx.lineTo(width, sy(cy + piece.slope_after * (x1 - cx)));

            if (options.keys && keys.length <= MAX_KEYS) {
                piece.keys.forEach(([kx, ky], i) => {
                    if (kx < x0 || kx > x1 || ky < y0 || ky > y1) return;
                    const [left, right] = piece.handles?.[i] ?? [null, null];
                    keys.push({ x: sx(kx), y: sy(ky), color: channel.color, left: pixel(left), right: pixel(right) });
                });
            }
        }
        ctx.stroke();
    }

    // handles: a line through the key to each handle, with a hollow end like Blender's
    if (keys.length <= MAX_KEYS && options.handles) {
        ctx.lineWidth = 1;
        ctx.strokeStyle = color("--graph-handle");
        ctx.beginPath();
        for (const key of keys) {
            for (const end of [key.left, key.right]) {
                if (!end) continue;
                ctx.moveTo(key.x, key.y);
                ctx.lineTo(end[0], end[1]);
            }
        }
        ctx.stroke();
        ctx.fillStyle = color("--graph-background");
        for (const key of keys) {
            for (const end of [key.left, key.right]) {
                if (!end) continue;
                ctx.beginPath();
                ctx.arc(end[0], end[1], 2.5, 0, Math.PI * 2);
                ctx.fill();
                ctx.stroke();
            }
        }
    }

    // keys on top of every curve
    if (keys.length <= MAX_KEYS) {
        ctx.lineWidth = 1;
        ctx.strokeStyle = color("--graph-background");
        for (const key of keys) {
            ctx.fillStyle = key.color;
            ctx.beginPath();
            ctx.arc(key.x, key.y, 3, 0, Math.PI * 2);
            ctx.fill();
            ctx.stroke();
        }
    }
    ctx.restore();
}

// sizes a canvas' backing store to its css size at `scale` (the screen's pixel ratio, times the zoom it's shown at),
// and draws in css pixels from there
export function fitCanvas(canvas: HTMLCanvasElement, ctx: CanvasRenderingContext2D, scale: number) {
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    if (canvas.width !== Math.round(width * scale) || canvas.height !== Math.round(height * scale)) {
        canvas.width = Math.round(width * scale);
        canvas.height = Math.round(height * scale);
    }
    ctx.setTransform(scale, 0, 0, scale, 0, 0);
    return { width, height };
}
