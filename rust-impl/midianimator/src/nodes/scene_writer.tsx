import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/nodeEntries";
import { useSetInputs } from "../utils/graphOps";

function scene_writer({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const nodeData = useNodeSpec("scene_writer");

    // unset is on, the spec's default
    const cleanKeyframes = data.inputs?.clean_keyframes ?? true;

    const cleanKeyframesComponent = (
        <label className="node-field flex items-center gap-1">
            <input type="checkbox" checked={cleanKeyframes} onChange={(e) => setInputs(id, { clean_keyframes: e.target.checked })} />
            Clean Keyframes
        </label>
    );

    const uiInject = {
        clean_keyframes: cleanKeyframesComponent,
    };

    const hiddenHandles = {
        clean_keyframes: true,
    };

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default scene_writer;
