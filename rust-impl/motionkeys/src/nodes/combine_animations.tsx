import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/nodeEntries";
import { useSetInputs } from "../utils/graphOps";
import { overlapModes } from "./animation_generator";
import CombinePreview from "../components/nodegraph/CombinePreview";
import { TILE_PICTURE } from "../components/nodegraph/simplified";

// how the generators combine, unset is the default, add. the note map's combine (NoteMapView.tsx) uses it too
export function OverlapSelect({ value, onChange, bare }: { value: string | undefined; onChange: (overlap: string) => void; bare?: boolean }) {
    return (
        <select className={bare ? "nodrag nopan" : "node-field nodrag nopan"} style={bare ? { flex: 1, minWidth: 0 } : undefined} value={value || "add"} onChange={(e) => onChange(e.target.value)}>
            {overlapModes.map((mode) => (
                <option key={mode.id} value={mode.id}>
                    {mode.name}
                </option>
            ))}
        </select>
    );
}

function combine_animations({ id, data }: { id: any; data: any }) {
    const nodeData = useNodeSpec("combine_animations");
    const setInputs = useSetInputs();
    const overlap = <OverlapSelect value={data?.inputs?.animation_overlap} onChange={(animation_overlap) => setInputs(id, { animation_overlap })} />;
    // its tile in the simplified view is its overlap in big text
    const tile = <CombinePreview overlap={data?.inputs?.animation_overlap} height={TILE_PICTURE} />;
    return <BaseNode nodeData={nodeData} inject={{ animation_overlap: overlap }} hidden={{ animation_overlap: true }} data={data} tile={tile} />;
}

export default combine_animations;
