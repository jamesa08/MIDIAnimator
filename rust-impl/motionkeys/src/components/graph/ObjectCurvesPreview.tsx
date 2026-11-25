import { useCallback, useMemo } from "react";
import { colorChannels } from "../../utils/curves";
import { ThemeColor, drawCurves, framed } from "../../utils/curveDrawing";
import { PreviewBox, usePreviewCanvas } from "./previewCanvas";
import { useValueCurves } from "./CurvePreview";

// keyframes by object (Evaluate Instrument's, Combine Keyframes') as a row of curves per object, all on one time and
// value scale so the rows compare. the curves are the backend's (graph_value_curves), each grouped under its object
function ObjectCurvesPreview({ outputs, height = 80 }: { outputs: any; height?: number }) {
    const channels = useValueCurves(outputs);
    const shown = useMemo(() => colorChannels([{ node: "", channels }]), [channels]);
    const objects = useMemo(() => [...new Set(shown.map((c) => c.group))], [shown]);

    const paint = useCallback(
        (ctx: CanvasRenderingContext2D, width: number, height: number, color: ThemeColor) => {
            ctx.fillStyle = color("--graph-background");
            ctx.fillRect(0, 0, width, height);
            const view = framed(shown, 0.02, 0.15);
            if (!view || objects.length === 0) return;
            const row = height / objects.length;
            objects.forEach((object, i) => {
                ctx.save();
                ctx.translate(0, i * row);
                if (i % 2 === 1) {
                    ctx.fillStyle = color("--graph-strip");
                    ctx.fillRect(0, 0, width, row);
                }
                ctx.beginPath();
                ctx.rect(0, 0, width, row);
                ctx.clip();
                drawCurves(
                    ctx,
                    { width, height: row, view },
                    shown.filter((c) => c.group === object),
                    { keys: false, handles: false, lineWidth: 1 },
                    color
                );
                ctx.restore();
            });
        },
        [shown, objects]
    );
    const canvasRef = usePreviewCanvas(paint);
    return <PreviewBox canvasRef={canvasRef} height={height} />;
}

export default ObjectCurvesPreview;
