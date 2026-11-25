import { RefObject, useCallback, useEffect, useRef } from "react";
import { useStore } from "@xyflow/react";
import { ThemeColor, fitCanvas } from "../../utils/curveDrawing";

// the most a node's picture is drawn zoomed in, past this it's left a little soft instead of using more memory
const MAX_ZOOM = 4;

// a node's picture on a canvas (its curves, its notes): drawn at the zoom the graph shows it at so it stays sharp, in
// steps so zooming doesn't redraw it every frame, and again when it resizes or the theme changes (its css is swapped in
// the head, utils/theme.ts). `paint` draws it in css pixels with the theme's colors
export function usePreviewCanvas(paint: (ctx: CanvasRenderingContext2D, width: number, height: number, color: ThemeColor) => void): RefObject<HTMLCanvasElement> {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const zoom = useStore((s) => Math.min(MAX_ZOOM, Math.ceil(s.transform[2] * 4) / 4));

    const draw = useCallback(() => {
        const canvas = canvasRef.current;
        const ctx = canvas?.getContext("2d");
        if (!canvas || !ctx) return;
        const { width, height } = fitCanvas(canvas, ctx, (window.devicePixelRatio || 1) * Math.max(zoom, 1));
        const style = getComputedStyle(canvas);
        paint(ctx, width, height, (name: string) => style.getPropertyValue(name).trim());
    }, [paint, zoom]);

    useEffect(draw, [draw]);

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

    return canvasRef;
}

// the box a picture's canvas is drawn in. the canvas is absolutely placed so its pixel size never counts toward the
// node's width, the node would grow with every draw otherwise (the canvas is as wide as the node, its pixels are wider
// than that by the zoom and the screen's pixel ratio)
export function PreviewBox({ canvasRef, height }: { canvasRef: RefObject<HTMLCanvasElement>; height: number }) {
    return (
        <div className="node-field relative border border-[var(--chrome-line)]" style={{ height }}>
            <canvas ref={canvasRef} className="absolute inset-0 block w-full h-full" />
        </div>
    );
}
