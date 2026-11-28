import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { NumberInput } from "../components/nodegraph/NodeFields";

function value({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const nodeData = useNodeSpec("value");

    return <BaseNode nodeData={nodeData} inject={{ value: <NumberInput id={id} input="value" label="Value" data={data} nodeData={nodeData} /> }} hidden={{ value: true }} data={data} />;
}

export default value;
