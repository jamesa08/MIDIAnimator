import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CurveChannel, colorChannels } from "../../utils/curves";
import { ThemeColor, View, drawCurves, drawGrid, framed } from "../../utils/curveDrawing";
import { PreviewBox, usePreviewCanvas } from "./previewCanvas";

// the view when there are no curves to frame
const EMPTY_VIEW: View = { x0: 0, x1: 1, y0: -1, y1: 1 };

// the curves in a node's last results (`outputs`), from the backend like the graph window's (graph_value_curves) so
// they're drawn the way Blender would. worked out again only when the results change, they come with every state update
export function useValueCurves(outputs: any): CurveChannel[] {
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
    return channels;
}

// a small graph of the curves in a node's last results (`outputs`), always framed to fit, just the curves on the grid.
// `curves` are drawn instead while they're given, a node's preview of a value that isn't set yet
function CurvePreview({ outputs, curves, height = 80 }: { outputs: any; curves?: CurveChannel[] | null; height?: number }) {
    const channels = useValueCurves(outputs);
    const shown = useMemo(() => colorChannels([{ node: "", channels: curves ?? channels }]), [curves, channels]);

    const paint = useCallback(
        (ctx: CanvasRenderingContext2D, width: number, height: number, color: ThemeColor) => {
            const plot = { width, height, view: framed(shown, 0.04, 0.15) ?? EMPTY_VIEW };
            ctx.fillStyle = color("--graph-background");
            ctx.fillRect(0, 0, width, height);
            drawGrid(ctx, plot, 1, color);
            drawCurves(ctx, plot, shown, { keys: false, handles: false }, color);
        },
        [shown]
    );
    const canvasRef = usePreviewCanvas(paint);
    return <PreviewBox canvasRef={canvasRef} height={height} />;
}

export default CurvePreview;
