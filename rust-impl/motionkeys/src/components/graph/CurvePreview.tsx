import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useStore } from "@xyflow/react";
import { CurveChannel, colorChannels } from "../../utils/curves";
import { View, drawCurves, drawGrid, fitCanvas, framed } from "../../utils/curveDrawing";

// the view when there are no curves to frame
const EMPTY_VIEW: View = { x0: 0, x1: 1, y0: -1, y1: 1 };
// the most the canvas is drawn zoomed in, past this it's left a little soft instead of using more memory
const MAX_ZOOM = 4;

// a small graph of the curves in a node's last results (`outputs`), always framed to fit, just the curves on the grid.
// the curves come from the backend like the graph window's (graph_value_curves), so they're drawn the way Blender would
function CurvePreview({ outputs, height = 80 }: { outputs: any; height?: number }) {
    const canvasRef = useRef<HTMLCanvasElement>(null);

    // worked out again only when the results change, they come with every state update
    const key = useMemo(() => JSON.stringify(outputs ?? null), [outputs]);
    const [channels, setChannels] = useState<CurveChannel[]>([]);
    useEffect(() => {
        if (outputs == null) {
            setChannels([]);
            return;
        }
        let current = true;
        invoke<CurveChannel[]>("graph_value_curves", { outputs })
            .then((next) => current && setChannels(next))
            .catch((e) => console.error(`Error getting preview curves: ${e}`));
        return () => {
            current = false;
        };
    }, [key]);
    const shown = useMemo(() => colorChannels([{ node: "", channels }]), [channels]);

    // drawn at the zoom the graph shows it at so it stays sharp, in steps so zooming doesn't redraw it every frame
    const zoom = useStore((s) => Math.min(MAX_ZOOM, Math.ceil(s.transform[2] * 4) / 4));

    const draw = useCallback(() => {
        const canvas = canvasRef.current;
        const ctx = canvas?.getContext("2d");
        if (!canvas || !ctx) return;
        const { width, height } = fitCanvas(canvas, ctx, (window.devicePixelRatio || 1) * Math.max(zoom, 1));
        const style = getComputedStyle(canvas);
        const color = (name: string) => style.getPropertyValue(name).trim();
        const plot = { width, height, view: framed(shown, 0.04, 0.15) ?? EMPTY_VIEW };

        ctx.fillStyle = color("--graph-background");
        ctx.fillRect(0, 0, width, height);
        drawGrid(ctx, plot, 1, color);
        drawCurves(ctx, plot, shown, { keys: false, handles: false }, color);
    }, [shown, zoom]);

    useEffect(draw, [draw]);

    // the node resizing and the theme changing (its css is swapped in the head, utils/theme.ts)
    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const resize = new ResizeObserver(draw);
        resize.observe(canvas);
        const theme = new MutationObserver(draw);
        theme.observe(document.head, { childList: true, subtree: true, characterData: true });
        return () => {
            resize.disconnect();
            theme.disconnect();
        };
    }, [draw]);

    // the canvas is absolutely placed so its pixel size never counts toward the node's width, the node would grow with
    // every draw otherwise (the canvas is as wide as the node, its pixels are wider than that by the zoom and the screen's
    // pixel ratio)
    return (
        <div className="node-field relative border border-[var(--chrome-line)]" style={{ height }}>
            <canvas ref={canvasRef} className="absolute inset-0 block w-full h-full" />
        </div>
    );
}

export default CurvePreview;
