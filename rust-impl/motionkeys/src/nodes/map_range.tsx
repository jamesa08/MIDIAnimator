import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { CheckboxInput } from "../components/nodegraph/NodeFields";

function map_range({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const nodeData = useNodeSpec("map_range");

    return <BaseNode nodeData={nodeData} inject={{ clamp: <CheckboxInput id={id} input="clamp" label="Clamp" data={data} nodeData={nodeData} /> }} hidden={{ clamp: true }} data={data} />;
}

export default map_range;
