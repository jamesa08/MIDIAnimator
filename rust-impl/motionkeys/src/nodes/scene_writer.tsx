import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { CheckboxRow } from "../components/nodegraph/NodeFields";
import { useNodeSpec } from "../utils/node";
import { useSetInputs } from "../utils/graphOps";

function scene_writer({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const nodeData = useNodeSpec("scene_writer");

    // unset is on, the spec's default
    const cleanKeyframes = data.inputs?.clean_keyframes ?? true;

    const cleanKeyframesComponent = <CheckboxRow label="Clean Keyframes" checked={cleanKeyframes} onChange={(checked) => setInputs(id, { clean_keyframes: checked })} />;

    const uiInject = {
        clean_keyframes: cleanKeyframesComponent,
    };

    const hiddenHandles = {
        clean_keyframes: true,
    };

    // keyframes set on it (over MCP) are a whole map, too much to show under the socket
    const hiddenValues = {
        keyframes: true,
    };

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} hideValues={hiddenValues} data={data} />;
}

export default scene_writer;
