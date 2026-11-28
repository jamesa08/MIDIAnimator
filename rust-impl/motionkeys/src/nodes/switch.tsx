import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { CheckboxInput, useConnectedInputs } from "../components/nodegraph/NodeFields";

function switch_node({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const nodeData = useNodeSpec("switch");
    const connected = useConnectedInputs(id);

    // the switch is ticked on the node, next to its socket, unless something is connected to it
    const toggle = connected.includes("switch") ? undefined : <CheckboxInput id={id} input="switch" label="Switch" data={data} nodeData={nodeData} />;

    return <BaseNode nodeData={nodeData} inject={{}} labels={{ switch: toggle }} hidden={{}} hideValues={{ switch: true }} data={data} />;
}

export default switch_node;
