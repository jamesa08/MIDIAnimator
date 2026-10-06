import { forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { ShownChannel, curveBounds } from "../../utils/curves";
import { View, drawCurves, drawGrid, fitCanvas, framed, gridLabel, toPixels } from "../../utils/curveDrawing";
import { useHold, useKeymap } from "../../utils/keymap";

// the strip along the bottom with the times
const TIME_STRIP = 20;
// the strip down the left with the values, dragging it scales the values like dragging the time strip scales time
const VALUE_STRIP = 40;
// how much the zoom buttons and keys zoom by
const ZOOM_STEP = 1.25;
// the view scaled by `scale` around a time and value
function zoomed(view: View, cx: number, cy: number, scaleX: number, scaleY: number): View {
    return { x0: cx - (cx - view.x0) * scaleX, x1: cx + (view.x1 - cx) * scaleX, y0: cy - (cy - view.y0) * scaleY, y1: cy + (view.y1 - cy) * scaleY };
}

// MARK: - Scroll bars

// Blender's scroll bars: a bar along the bottom of the curves for time and one down the right for values. each shows the
// view's part of everything there is to see (the curves and the view), dragging the middle pans and dragging an end
// moves that end of the view, which scales the axis. clicking the bar off the thumb moves the view there
const BAR = 8;
const BAR_INSET = 4;
// the thumb is never shorter than this, so its ends can still be grabbed
const MIN_THUMB = 26;

type Bounds = { x0: number; x1: number; y0: number; y1: number };
// a bar from `start` to `end` along its axis and `across` from the top or left, its thumb from `a0` to `a1`.
// `perPixel` is how much time or value a pixel along it is
type Bar = { axis: "x" | "y"; start: number; end: number; across: number; a0: number; a1: number; perPixel: number };
type BarPart = "start" | "end" | "thumb" | "track";

function scrollBars(width: number, plot: number, view: View, bounds: Bounds | null): Bar[] {
    const total = bounds ? { x0: Math.min(bounds.x0, view.x0), x1: Math.max(bounds.x1, view.x1), y0: Math.min(bounds.y0, view.y0), y1: Math.max(bounds.y1, view.y1) } : view;
    const bar = (axis: "x" | "y", start: number, end: number, across: number, range: number, f0: number, f1: number): Bar => {
        const length = Math.max(end - start, 1);
        let a0 = start + f0 * length;
        let a1 = start + f1 * length;
        if (a1 - a0 < MIN_THUMB) {
            const middle = (a0 + a1) / 2;
            a0 = middle - MIN_THUMB / 2;
            a1 = middle + MIN_THUMB / 2;
        }
        return { axis, start, end, across, a0, a1, perPixel: range / length };
    };
    const rangeX = total.x1 - total.x0;
    const rangeY = total.y1 - total.y0;
    return [
        bar("x", VALUE_STRIP, width - BAR - BAR_INSET * 2, plot - BAR - BAR_INSET, rangeX, (view.x0 - total.x0) / rangeX, (view.x1 - total.x0) / rangeX),
        // values go up, the bar goes down
        bar("y", BAR_INSET, plot - BAR - BAR_INSET * 2, width - BAR - BAR_INSET, rangeY, (total.y1 - view.y1) / rangeY, (total.y1 - view.y0) / rangeY),
    ];
}

// the bar and the part of it under a point, a few pixels of slack around it
function barAt(bars: Bar[], x: number, y: number): { bar: Bar; part: BarPart } | null {
    for (const bar of bars) {
        const along = bar.axis === "x" ? x : y;
        const across = bar.axis === "x" ? y : x;
        if (across < bar.across - 3 || across > bar.across + BAR + 3 || along < bar.start - 3 || along > bar.end + 3) continue;
        if (Math.abs(along - bar.a0) <= BAR) return { bar, part: "start" };
        if (Math.abs(along - bar.a1) <= BAR) return { bar, part: "end" };
        return { bar, part: along > bar.a0 && along < bar.a1 ? "thumb" : "track" };
    }
    return null;
}

// what a drag that starts at a point does: pans in the curves, scales time in the time strip and values in the value strip
type DragZone = "curves" | "time" | "value";
function dragZone(x: number, y: number, height: number): DragZone {
    if (y >= height - TIME_STRIP) return "time";
    if (x < VALUE_STRIP) return "value";
    return "curves";
}
const ZONE_CURSORS: Record<DragZone, string> = { curves: "", time: "ew-resize", value: "ns-resize" };

// what the graph window's toolbar can do with the view
export type CurveCanvasHandle = { zoomIn: () => void; zoomOut: () => void; frameAll: () => void };

type CurveCanvasProps = { channels: ShownChannel[]; frameKey: string; fps: number; frames: boolean; showHandles: boolean };

// curves drawn on a time and value grid: wheel zooms, middle drag (or the pan key held and drag) pans, ctrl middle
// drag stretches each axis, dragging the time or value strip scales that axis. times show in seconds or, with
// `frames`, in frames at `fps`. `frameKey` changing frames the channels once they're there

const CurveCanvas = forwardRef<CurveCanvasHandle, CurveCanvasProps>(function CurveCanvas({ channels, frameKey, fps, frames, showHandles }, ref) {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const view = useRef<View>({ x0: -0.5, x1: 4.5, y0: -1.2, y1: 1.2 });
    const channelsRef = useRef(channels);
    channelsRef.current = channels;
    // everything there is to see, for the scroll bars
    const bounds = useMemo(() => curveBounds(channels), [channels]);
    const boundsRef = useRef(bounds);
    boundsRef.current = bounds;
    // seconds are multiplied by this to show them, the fps when they show in frames
    const timeScale = useRef(1);
    timeScale.current = frames ? fps : 1;
    const showHandlesRef = useRef(showHandles);
    showHandlesRef.current = showHandles;
    const framedFor = useRef<string | null>(null);
    const panHeld = useHold("graph_editor", "pan");

    // MARK: Drawing

    const pending = useRef(false);
    const draw = useCallback(() => {
        pending.current = false;
        const canvas = canvasRef.current;
        const ctx = canvas?.getContext("2d");
        if (!canvas || !ctx) return;
        const { width, height } = fitCanvas(canvas, ctx, window.devicePixelRatio || 1);
        const style = getComputedStyle(canvas);
        const color = (name: string) => style.getPropertyValue(name).trim();
        const plot = { width, height: height - TIME_STRIP, view: view.current };
        const { sx, sy } = toPixels(plot);

        ctx.fillStyle = color("--graph-background");
        ctx.fillRect(0, 0, width, height);

        const scale = timeScale.current;
        const inFrames = scale !== 1;
        const { timeLines, stepX, valueLines, stepY } = drawGrid(ctx, plot, scale, color);
        drawCurves(ctx, plot, channelsRef.current, { keys: true, handles: showHandlesRef.current }, color);

        // value labels down the left, time labels in the strip
        ctx.font = "11px Arial, sans-serif";
        ctx.fillStyle = color("--graph-text");
        ctx.textBaseline = "bottom";
        ctx.textAlign = "left";
        for (const y of valueLines) {
            ctx.fillText(gridLabel(y, stepY), 4, Math.round(sy(y)) - 2);
        }
        ctx.fillStyle = color("--graph-strip");
        ctx.fillRect(0, plot.height, width, TIME_STRIP);
        ctx.lineWidth = 1;
        ctx.strokeStyle = color("--graph-grid");
        ctx.beginPath();
        ctx.moveTo(0, plot.height + 0.5);
        ctx.lineTo(width, plot.height + 0.5);
        ctx.stroke();
        ctx.fillStyle = color("--graph-text");
        ctx.textBaseline = "middle";
        ctx.textAlign = "center";
        for (const t of timeLines) {
            ctx.fillText(inFrames ? gridLabel(t, stepX) : `${gridLabel(t, stepX)}s`, sx(t / scale), plot.height + TIME_STRIP / 2);
        }

        // scroll bars on top: the thumb with a round grip at each end
        for (const bar of scrollBars(width, plot.height, view.current, boundsRef.current)) {
            const horizontal = bar.axis === "x";
            ctx.fillStyle = color("--graph-scrollbar");
            ctx.beginPath();
            if (horizontal) ctx.roundRect(bar.a0, bar.across, bar.a1 - bar.a0, BAR, BAR / 2);
            else ctx.roundRect(bar.across, bar.a0, BAR, bar.a1 - bar.a0, BAR / 2);
            ctx.fill();
            ctx.fillStyle = color("--graph-background");
            ctx.strokeStyle = color("--graph-scrollbar-grip");
            ctx.lineWidth = 1.5;
            for (const end of [bar.a0 + BAR / 2, bar.a1 - BAR / 2]) {
                ctx.beginPath();
                if (horizontal) ctx.arc(end, bar.across + BAR / 2, BAR / 2 - 0.5, 0, Math.PI * 2);
                else ctx.arc(bar.across + BAR / 2, end, BAR / 2 - 0.5, 0, Math.PI * 2);
                ctx.fill();
                ctx.stroke();
            }
        }
    }, []);

    // drawn on the next frame. macOS can hold back frames from a window of an app in the background, so a timer draws it
    // if no frame has come (the run updating while Blender is in front)
    const redraw = useCallback(() => {
        if (pending.current) return;
        pending.current = true;
        const frame = requestAnimationFrame(draw);
        setTimeout(() => {
            if (!pending.current) return;
            cancelAnimationFrame(frame);
            draw();
        }, 100);
    }, [draw]);

    const frameAll = useCallback(() => {
        const next = framed(channelsRef.current);
        if (!next) return;
        view.current = next;
        redraw();
    }, [redraw]);

    // new channels are drawn where the view is, a new frame key frames them once they've come
    useEffect(() => {
        if (framedFor.current !== frameKey && channels.length > 0) {
            framedFor.current = frameKey;
            frameAll();
        } else {
            redraw();
        }
    }, [channels, frameKey, frameAll, redraw]);

    // the window resizing and the theme changing (its css is swapped in the head, utils/theme.ts)
    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        // drawn right away, before the resized canvas is shown (a window drag shows each step as soon as it's laid out)
        const resize = new ResizeObserver(draw);
        resize.observe(canvas);
        const theme = new MutationObserver(redraw);
        theme.observe(document.head, { childList: true, subtree: true, characterData: true });
        const unlisten = listen("settings_changed", redraw);
        return () => {
            resize.disconnect();
            theme.disconnect();
            unlisten.then((f) => f());
        };
    }, [draw, redraw]);

    // the units change what's drawn, not the view
    useEffect(redraw, [frames, fps, showHandles, redraw]);

    // zooms both axes around the middle of the curves
    const zoom = useCallback(
        (scale: number) => {
            const { x0, x1, y0, y1 } = view.current;
            view.current = zoomed(view.current, (x0 + x1) / 2, (y0 + y1) / 2, scale, scale);
            redraw();
        },
        [redraw]
    );

    const zoomIn = useCallback(() => zoom(1 / ZOOM_STEP), [zoom]);
    const zoomOut = useCallback(() => zoom(ZOOM_STEP), [zoom]);
    useImperativeHandle(ref, () => ({ zoomIn, zoomOut, frameAll }), [zoomIn, zoomOut, frameAll]);
    useKeymap("graph_editor", { frame_all: frameAll, zoom_in: zoomIn, zoom_out: zoomOut });

    // MARK: Navigation

    // zooms around the cursor, pinching too (a wheel event with ctrl)
    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const handleWheel = (event: WheelEvent) => {
            event.preventDefault();
            const rect = canvas.getBoundingClientRect();
            const plot = rect.height - TIME_STRIP;
            const { x0, x1, y0, y1 } = view.current;
            const fx = (event.clientX - rect.left) / rect.width;
            const fy = 1 - (event.clientY - rect.top) / plot;
            const cx = x0 + fx * (x1 - x0);
            const cy = y0 + fy * (y1 - y0);
            const delta = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? event.deltaY * 16 : event.deltaY;
            const scale = Math.exp(delta * (event.ctrlKey ? 0.01 : 0.002));
            view.current = zoomed(view.current, cx, cy, scale, scale);
            redraw();
        };
        canvas.addEventListener("wheel", handleWheel, { passive: false });
        return () => canvas.removeEventListener("wheel", handleWheel);
    }, [redraw]);

    // a left drag on a scroll bar: the middle pans, an end moves that end of the view
    const dragBar = (event: React.PointerEvent<HTMLCanvasElement>, bar: Bar, part: BarPart) => {
        const canvas = event.currentTarget;
        const rect = canvas.getBoundingClientRect();
        const horizontal = bar.axis === "x";
        const along = horizontal ? event.clientX - rect.left : event.clientY - rect.top;
        // values run the other way down the bar
        const sign = horizontal ? 1 : -1;
        // the view can't get narrower than a few pixels of the bar
        const minRange = bar.perPixel * 4;
        let start = { ...view.current };
        // off the thumb moves its middle there, then drags it from there
        if (part === "track") {
            const shift = (along - (bar.a0 + bar.a1) / 2) * bar.perPixel * sign;
            start = horizontal ? { ...start, x0: start.x0 + shift, x1: start.x1 + shift } : { ...start, y0: start.y0 + shift, y1: start.y1 + shift };
            view.current = start;
            redraw();
        }
        const grabbed = part === "track" ? "thumb" : part;
        const startPos = horizontal ? event.clientX : event.clientY;

        const handleMove = (e: PointerEvent) => {
            const moved = ((horizontal ? e.clientX : e.clientY) - startPos) * bar.perPixel * sign;
            const next = { ...start };
            const [lo, hi] = horizontal ? (["x0", "x1"] as const) : (["y0", "y1"] as const);
            // the bar's start is the low end of time (the left) but the high end of values (the top)
            const moves = grabbed === "thumb" ? "both" : (grabbed === "start") === horizontal ? "lo" : "hi";
            if (moves === "both") {
                next[lo] = start[lo] + moved;
                next[hi] = start[hi] + moved;
            } else if (moves === "lo") {
                next[lo] = Math.min(start[lo] + moved, start[hi] - minRange);
            } else {
                next[hi] = Math.max(start[hi] + moved, start[lo] + minRange);
            }
            view.current = next;
            redraw();
        };
        const handleUp = () => {
            canvas.removeEventListener("pointermove", handleMove);
            canvas.removeEventListener("pointerup", handleUp);
            canvas.removeEventListener("pointercancel", handleUp);
        };
        event.preventDefault();
        canvas.setPointerCapture(event.pointerId);
        canvas.addEventListener("pointermove", handleMove);
        canvas.addEventListener("pointerup", handleUp);
        canvas.addEventListener("pointercancel", handleUp);
    };

    // the scroll bar part under the mouse, if any
    const barUnder = (event: React.PointerEvent<HTMLCanvasElement>) => {
        const rect = event.currentTarget.getBoundingClientRect();
        return barAt(scrollBars(rect.width, rect.height - TIME_STRIP, view.current, boundsRef.current), event.clientX - rect.left, event.clientY - rect.top);
    };

    const handlePointerDown = (event: React.PointerEvent<HTMLCanvasElement>) => {
        const canvas = event.currentTarget;
        const rect = canvas.getBoundingClientRect();
        const hit = event.button === 0 && !panHeld ? barUnder(event) : null;
        if (hit) {
            dragBar(event, hit.bar, hit.part);
            return;
        }
        // a left drag on the time or value strip scales that axis
        const zone = dragZone(event.clientX - rect.left, event.clientY - rect.top, rect.height);
        const scaleAxis = event.button === 0 && !panHeld && zone !== "curves";
        const pan = event.button === 1 || (event.button === 0 && panHeld);
        if (!pan && !scaleAxis) return;
        event.preventDefault();
        canvas.setPointerCapture(event.pointerId);
        const start = { ...view.current };
        const startX = event.clientX;
        const startY = event.clientY;
        // ctrl stretches the axes instead, around where the drag started (Blender's ctrl middle drag)
        const stretch = scaleAxis || (event.ctrlKey && event.button === 1);
        const anchorX = start.x0 + ((startX - rect.left) / rect.width) * (start.x1 - start.x0);
        const anchorY = start.y0 + (1 - (startY - rect.top) / (rect.height - TIME_STRIP)) * (start.y1 - start.y0);

        const handleMove = (e: PointerEvent) => {
            const dx = e.clientX - startX;
            const dy = e.clientY - startY;
            if (stretch) {
                // dragging right or up zooms in, a strip only scales its own axis
                const sx = scaleAxis && zone === "value" ? 1 : Math.exp(-dx * 0.005);
                const sy = scaleAxis && zone === "time" ? 1 : Math.exp(dy * 0.005);
                view.current = zoomed(start, anchorX, anchorY, sx, sy);
            } else {
                const mx = (dx / rect.width) * (start.x1 - start.x0);
                const my = (dy / (rect.height - TIME_STRIP)) * (start.y1 - start.y0);
                view.current = { x0: start.x0 - mx, x1: start.x1 - mx, y0: start.y0 + my, y1: start.y1 + my };
            }
            redraw();
        };
        const handleUp = () => {
            canvas.removeEventListener("pointermove", handleMove);
            canvas.removeEventListener("pointerup", handleUp);
            canvas.removeEventListener("pointercancel", handleUp);
        };
        canvas.addEventListener("pointermove", handleMove);
        canvas.addEventListener("pointerup", handleUp);
        canvas.addEventListener("pointercancel", handleUp);
    };

    // the strips show they can be dragged
    const handlePointerMove = (event: React.PointerEvent<HTMLCanvasElement>) => {
        const canvas = event.currentTarget;
        const rect = canvas.getBoundingClientRect();
        const hit = panHeld ? null : barUnder(event);
        if (hit) {
            // the ends scale like the strips do
            const ends = hit.part === "start" || hit.part === "end";
            canvas.style.cursor = ends ? (hit.bar.axis === "x" ? "ew-resize" : "ns-resize") : "";
            return;
        }
        canvas.style.cursor = panHeld ? "grab" : ZONE_CURSORS[dragZone(event.clientX - rect.left, event.clientY - rect.top, rect.height)];
    };

    return <canvas ref={canvasRef} className="w-full h-full block" style={{ cursor: panHeld ? "grab" : undefined }} onPointerDown={handlePointerDown} onPointerMove={handlePointerMove} />;
});

export default CurveCanvas;
