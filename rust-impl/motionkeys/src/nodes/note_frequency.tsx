import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/nodeEntries";

function note_frequency({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const nodeData = useNodeSpec("note_frequency");

    return <BaseNode nodeData={nodeData} inject={{}} hidden={{}} data={data} />;
}

export default note_frequency;
