import "@xyflow/react/dist/base.css";
import { useNodeId } from "@xyflow/react";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { useStateContext } from "../contexts/StateContext";
import ObjectCurvesPreview from "../components/graph/ObjectCurvesPreview";
import { TILE_PICTURE, keyframesByObject } from "../components/nodegraph/simplified";

// a node that only shows its spec's sockets, for node types with nothing to set or show. one whose last run gave
// keyframes by object (Combine Keyframes) pictures a curve per object on its tile in the simplified view
function SpecNode({ nodeType, data }: { nodeType: string; data: any }) {
    const id = useNodeId();
    const { backEndState: state } = useStateContext();
    const preview = data === "preview" || data?.preview === true;
    const results = preview || !id ? undefined : state?.executed_results?.[id];
    const keyframes = keyframesByObject(results) ? results : undefined;
    return <BaseNode nodeData={useNodeSpec(nodeType)} data={data} tile={keyframes && <ObjectCurvesPreview outputs={keyframes} height={TILE_PICTURE} />} />;
}

export default SpecNode;
