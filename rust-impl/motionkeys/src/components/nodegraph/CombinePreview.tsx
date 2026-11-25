import { overlapModes } from "../../nodes/animation_generator";

// Combine Animations' picture on its tile in the simplified view: its overlap, what its dropdown shows, in big text.
// the long names (Rest Value Crossing) wrap
function CombinePreview({ overlap, height = 80 }: { overlap: string | undefined; height?: number }) {
    const mode = overlapModes.find((m) => m.id === (overlap || "add"))?.name ?? overlap;
    return (
        <div className="node-field flex items-center justify-center text-center border border-[var(--chrome-line)] overflow-hidden" style={{ height, background: "var(--graph-background)", fontSize: 30, fontWeight: "bold", lineHeight: 1.1 }}>
            {mode}
        </div>
    );
}

export default CombinePreview;
