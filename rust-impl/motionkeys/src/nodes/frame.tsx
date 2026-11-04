import { CSSProperties, useState } from "react";
import { NodeResizeControl, useStoreApi } from "@xyflow/react";
import { useGroupContext } from "../contexts/GroupContext";
import { useGraphOps } from "../utils/graphOps";
import { FRAME_COLOR, isLocked, labelHeight, labelSize } from "../utils/frames";
import { addsToSelection } from "../utils/keymap";

const SIDES = ["top", "right", "bottom", "left"] as const;
// where the label goes along the top
const ALIGN: Record<string, string> = { left: "flex-start", center: "center", right: "flex-end" };

// a frame (src/utils/frames.ts): a box in its color under the nodes in it, its label along the top in its size. it's
// moved by its label strip, renamed by double clicking it and resized by its sides, a locked one only renamed. the
// editor sizes it and selects it
function Frame({ id, data, selected }: { id: string; data: any; selected?: boolean }) {
    const { scopeId, editable } = useGroupContext();
    const { apply } = useGraphOps(scopeId);
    const store = useStoreApi();
    const [renaming, setRenaming] = useState(false);
    const label: string = data?.label ?? "";
    const frame = { type: "frame", data };

    // the same rename as a node's label, empty is no label
    const rename = (value: string) => {
        setRenaming(false);
        if (value.trim() !== label) apply([{ op: "set_label", node: id, label: value.trim() }]).catch((e) => console.error(`set_label on ${id}: ${e}`));
    };

    // pressing the label selects the frame alone before a drag can start, so it's dragged without what else was selected
    const press = (event: React.MouseEvent) => {
        if (event.button === 0 && !selected && !addsToSelection(event) && store.getState().elementsSelectable) store.getState().addSelectedNodes([id]);
    };

    return (
        <div className="graph-frame" style={{ "--frame-color": data?.color ?? FRAME_COLOR } as CSSProperties}>
            <div className="graph-frame-label" style={{ height: labelHeight(frame), fontSize: labelSize(frame), justifyContent: ALIGN[data?.label_align] ?? ALIGN.left }} onMouseDown={press} onDoubleClick={() => editable && setRenaming(true)}>
                {renaming ? (
                    <input
                        className="nodrag nopan graph-frame-rename"
                        autoFocus
                        defaultValue={label}
                        onBlur={(e) => rename(e.target.value)}
                        onKeyDown={(e) => {
                            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                            if (e.key === "Escape") setRenaming(false);
                            e.stopPropagation();
                        }}
                    />
                ) : (
                    <span>{label}</span>
                )}
            </div>
            {editable && !isLocked(frame) && SIDES.map((side) => <NodeResizeControl key={side} position={side} variant={"line" as any} minWidth={100} minHeight={labelHeight(frame) + 20} />)}
        </div>
    );
}

export default Frame;
