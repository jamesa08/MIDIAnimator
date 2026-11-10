import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useStore } from "@xyflow/react";
import { CurveChannel, colorChannels } from "../../utils/curves";
import { drawCurves, drawGrid, fitCanvas, framed, toPixels } from "../../utils/curveDrawing";
import { useInputDrag } from "../../utils/graphOps";
import { CLICK_DISTANCE } from "../nodegraph/NodeGraphCanvas";

type Point = "windup" | "hit" | "recoil";
type Points = Record<Point, number>;
const POINTS: { id: Point; name: string; color: string }[] = [
    { id: "windup", name: "Windup", color: "--phase-windup" },
    { id: "hit", name: "Hit", color: "--phase-hit" },
    { id: "recoil", name: "Recoil", color: "--phase-recoil" },
];
// frames kept between two points
const GAP = 0.5;
// room above the curve for the lines' grips, pixels
const GRIP = 12;
// how close to a line a press picks it up, pixels
const PICK = 8;
const HEIGHT = 130;
// the most the canvas is drawn zoomed in, past this it's left a little soft instead of using more memory
const MAX_ZOOM = 4;

// momentum's windup, hit and recoil on a generator's first note on curve, in the curve's frames: a line for each over
// the curve and a row for each under it, both dragged to move it, a row double clicked to type its frame. the hit is the
// note on anchor point (it can't move while that's connected), the windup and recoil are kept as frames from it.
// `generator` is the node's last output, with the points it found where they aren't set (`set` says which are). reset
// finds them again
function PhaseEditor({ id, generator, fps, anchorConnected, set }: { id: string; generator: any; fps: number; anchorConnected: boolean; set: { windup: boolean; recoil: boolean } }) {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const drag = useInputDrag(id);

    // the curve, drawn the way Blender would like the node's preview
    const curve = useMemo(() => (generator?.note_on_keyframes ?? []).find((c: any) => c?.keys?.length > 0) ?? null, [generator]);
    const key = useMemo(() => JSON.stringify(curve), [curve]);
    const [channels, setChannels] = useState<CurveChannel[]>([]);
    useEffect(() => {
        if (curve == null) {
            setChannels([]);
            return;
        }
        let current = true;
        invoke<CurveChannel[]>("graph_value_curves", { outputs: { curve } })
            .then((next) => current && setChannels(next))
            .catch((e) => console.error(`Error getting the phase curve: ${e}`));
        return () => {
            current = false;
        };
    }, [key]);
    const shown = useMemo(() => colorChannels([{ node: "", channels }]), [channels]);

    // the curve's first and last frames, the points stay between them
    const range = useMemo(() => {
        const keys = channels.flatMap((channel) => channel.pieces.flatMap((piece) => piece.keys));
        if (keys.length === 0) return null;
        return { start: Math.min(...keys.map((k) => k[0])) * fps, end: Math.max(...keys.map((k) => k[0])) * fps };
    }, [channels, fps]);

    // where the points are, the dragged one where it's dragged to until the node's output catches up
    const hit = -(generator?.note_on_anchor_point ?? 0);
    const found: Points = { windup: hit + (generator?.overlap_windup ?? 0), hit, recoil: hit + (generator?.overlap_recoil ?? 0) };
    const [moved, setMoved] = useState<Partial<Points> | null>(null);
    const points: Points = { ...found, ...(moved ?? {}) };
    useEffect(() => {
        if (!drag.dragging()) setMoved(null);
    }, [generator]);

    // a point moved to frame `f`, on the curve and always in order: the windup before the hit and the recoil after it
    // win over the curve's ends (a curve keyed from its hit has its windup before its start). the hit is the anchor
    // point, the windup and recoil are frames from it, so moving the hit sets them too, where they are on the curve.
    // found again around the new hit, a recoil would jump to the old hit's turn
    const move = (point: Point, f: number) => {
        if (!range || !Number.isFinite(f)) return;
        f = Math.max(range.start, Math.min(range.end, Math.round(f * 10) / 10));
        const next: Points = { ...points, [point]: f };
        if (point === "hit") next.hit = Math.max(points.windup + GAP, Math.min(points.recoil - GAP, f));
        // the node can't take a windup that isn't before the hit or a recoil that isn't after it
        const windup = Math.min(next.windup - next.hit, -GAP);
        const recoil = Math.max(next.recoil - next.hit, GAP);
        if (point === "windup") drag.send({ overlap_windup: windup });
        if (point === "recoil") drag.send({ overlap_recoil: recoil });
        if (point === "hit") drag.send({ note_on_anchor_point: -next.hit, overlap_windup: windup, overlap_recoil: recoil });
        // shown where they were sent until the output catches up
        setMoved({ windup: next.hit + windup, hit: next.hit, recoil: next.hit + recoil });
    };
    const movable = (point: Point) => point !== "hit" || !anchorConnected;

    // drawn at the zoom the graph shows it at so it stays sharp, in steps so zooming doesn't redraw it every frame
    const zoom = useStore((s) => Math.min(MAX_ZOOM, Math.ceil(s.transform[2] * 4) / 4));
    const plotRef = useRef<{ x: (frame: number) => number; frame: (x: number) => number } | null>(null);
    const draw = useCallback(() => {
        const canvas = canvasRef.current;
        const ctx = canvas?.getContext("2d");
        if (!canvas || !ctx) return;
        const { width, height } = fitCanvas(canvas, ctx, (window.devicePixelRatio || 1) * Math.max(zoom, 1));
        const style = getComputedStyle(canvas);
        const color = (name: string) => style.getPropertyValue(name).trim();
        ctx.fillStyle = color("--graph-background");
        ctx.fillRect(0, 0, width, height);
        const view = framed(shown, 0.04, 0.12);
        if (!view) return;
        // the curve under the grips
        const plot = { width, height: height - GRIP, view };
        ctx.save();
        ctx.translate(0, GRIP);
        drawGrid(ctx, plot, fps, color);
        drawCurves(ctx, plot, shown, { keys: true, handles: false }, color);
        ctx.restore();
        const { sx } = toPixels(plot);
        plotRef.current = { x: (frame) => sx(frame / fps), frame: (x) => (view.x0 + (x / width) * (view.x1 - view.x0)) * fps };
        // a line for each point with a grip on top
        for (const { id: point, color: name } of POINTS) {
            const x = Math.round(sx(points[point] / fps)) + 0.5;
            ctx.strokeStyle = ctx.fillStyle = color(name);
            ctx.lineWidth = 2;
            ctx.beginPath();
            ctx.moveTo(x, GRIP);
            ctx.lineTo(x, height);
            ctx.stroke();
            ctx.beginPath();
            ctx.moveTo(x - 5, 1);
            ctx.lineTo(x + 5, 1);
            ctx.lineTo(x, GRIP);
            ctx.closePath();
            ctx.fill();
        }
    }, [shown, zoom, fps, points.windup, points.hit, points.recoil]);
    useEffect(draw, [draw]);
    const drawRef = useRef(draw);
    drawRef.current = draw;

    // the node resizing and the theme changing (its css is swapped in the head, utils/theme.ts). watched once, a new
    // observer every move during a drag would draw each frame twice
    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const resize = new ResizeObserver(() => drawRef.current());
        resize.observe(canvas);
        const theme = new MutationObserver(() => drawRef.current());
        theme.observe(document.head, { childList: true, subtree: true, characterData: true });
        return () => {
            resize.disconnect();
            theme.disconnect();
        };
    }, []);

    // a press on the curve picks up the nearest line
    const grabRef = useRef<Point | null>(null);
    const nearest = (x: number): Point | null => {
        const plot = plotRef.current;
        if (!plot) return null;
        let best: Point | null = null;
        let distance = PICK;
        for (const { id: point } of POINTS) {
            const d = Math.abs(plot.x(points[point]) - x);
            if (d < distance && movable(point)) {
                distance = d;
                best = point;
            }
        }
        return best;
    };
    const localX = (e: React.PointerEvent<HTMLElement>) => {
        const rect = e.currentTarget.getBoundingClientRect();
        // the node is scaled with the graph's zoom, the canvas draws in its own css pixels
        return ((e.clientX - rect.left) / rect.width) * e.currentTarget.offsetWidth;
    };

    return (
        <div className="node-field phase-editor nodrag nopan">
            <div className="phase-curve" style={{ height: HEIGHT }}>
                <canvas
                    ref={canvasRef}
                    className="absolute inset-0 block w-full h-full"
                    onPointerDown={(e) => {
                        const point = nearest(localX(e));
                        if (!point) return;
                        e.currentTarget.setPointerCapture(e.pointerId);
                        grabRef.current = point;
                        drag.start();
                    }}
                    onPointerMove={(e) => {
                        const x = localX(e);
                        const point = grabRef.current;
                        if (point && plotRef.current) {
                            move(point, plotRef.current.frame(x));
                            return;
                        }
                        e.currentTarget.style.cursor = nearest(x) ? "ew-resize" : "default";
                    }}
                    onLostPointerCapture={() => {
                        if (!grabRef.current) return;
                        grabRef.current = null;
                        drag.stop();
                    }}
                />
            </div>
            <div className="phase-rows">
                {POINTS.map(({ id: point, name, color }) => (
                    <PhaseRow key={point} name={name} color={color} frame={points[point]} range={range} disabled={!movable(point)} onStart={drag.start} onMove={(f) => move(point, f)} onEnd={drag.stop} />
                ))}
            </div>
            <button className="phase-reset bg-transparent border border-black rounded" disabled={!set.windup && !set.recoil} onClick={() => drag.send({ overlap_windup: null, overlap_recoil: null })}>
                Reset
            </button>
        </div>
    );
}

// a point's row: its name and frame over a bar filled as far along the curve as it is. dragged across it moves the
// point, a double click types its frame in
function PhaseRow({ name, color, frame, range, disabled, onStart, onMove, onEnd }: { name: string; color: string; frame: number; range: { start: number; end: number } | null; disabled: boolean; onStart: () => void; onMove: (frame: number) => void; onEnd: () => void }) {
    const [typing, setTyping] = useState(false);
    const cancelRef = useRef(false);
    const pressRef = useRef<{ x: number; dragging: boolean } | null>(null);
    const fill = range && range.end > range.start ? ((frame - range.start) / (range.end - range.start)) * 100 : 0;
    const style = { "--phase": `var(${color})` } as React.CSSProperties;
    const at = (e: React.PointerEvent<HTMLDivElement>) => {
        if (!range) return;
        const rect = e.currentTarget.getBoundingClientRect();
        onMove(range.start + ((e.clientX - rect.left) / rect.width) * (range.end - range.start));
    };
    if (typing) {
        const commit = (text: string) => {
            setTyping(false);
            const cancelled = cancelRef.current;
            cancelRef.current = false;
            if (!cancelled && text.trim() !== "" && !isNaN(Number(text))) onMove(Number(text));
        };
        return (
            <div className="phase-row" style={style}>
                <input
                    autoFocus
                    defaultValue={frame.toFixed(1)}
                    onFocus={(e) => e.target.select()}
                    onBlur={(e) => commit(e.target.value)}
                    onKeyDown={(e) => {
                        if (e.key === "Enter") e.currentTarget.blur();
                        if (e.key === "Escape") {
                            cancelRef.current = true;
                            e.currentTarget.blur();
                        }
                    }}
                />
            </div>
        );
    }
    return (
        <div
            className={`phase-row${disabled ? " phase-row-disabled" : ""}`}
            style={style}
            onPointerDown={(e) => {
                if (disabled) return;
                e.currentTarget.setPointerCapture(e.pointerId);
                pressRef.current = { x: e.clientX, dragging: false };
            }}
            onPointerMove={(e) => {
                const press = pressRef.current;
                if (!press || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
                // a drag once it's moved further than a click can, from then on it follows the pointer
                if (!press.dragging) {
                    if (Math.abs(e.clientX - press.x) <= CLICK_DISTANCE) return;
                    press.dragging = true;
                    onStart();
                }
                at(e);
            }}
            onLostPointerCapture={() => {
                const press = pressRef.current;
                pressRef.current = null;
                if (press?.dragging) onEnd();
            }}
            onDoubleClick={() => {
                if (disabled) return;
                cancelRef.current = false;
                setTyping(true);
            }}
        >
            <div className="phase-row-fill" style={{ width: `${Math.max(0, Math.min(100, fill))}%` }} />
            <span>{name}</span>
            <span>{frame.toFixed(1)}</span>
        </div>
    );
}

export default PhaseEditor;
