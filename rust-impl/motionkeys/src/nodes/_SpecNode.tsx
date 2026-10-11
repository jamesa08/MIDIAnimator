import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/nodeEntries";

// a node that only shows its spec's sockets, for node types with nothing to set or show
function SpecNode({ nodeType, data }: { nodeType: string; data: any }) {
    return <BaseNode nodeData={useNodeSpec(nodeType)} data={data} />;
}

export default SpecNode;
