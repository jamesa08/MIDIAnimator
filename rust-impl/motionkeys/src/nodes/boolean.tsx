import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { CheckboxInput } from "../components/nodegraph/NodeFields";

function boolean({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const nodeData = useNodeSpec("boolean");

    return <BaseNode nodeData={nodeData} inject={{ boolean: <CheckboxInput id={id} input="boolean" label="Boolean" data={data} nodeData={nodeData} /> }} hidden={{ boolean: true }} data={data} />;
}

export default boolean;
