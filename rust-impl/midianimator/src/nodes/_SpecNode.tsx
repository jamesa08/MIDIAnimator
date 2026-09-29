import { useContext, useEffect, useState } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { getNodeData } from "../utils/node";
import { StateContext } from "../contexts/StateContext";

// a node that only shows its spec's sockets, for node types with nothing to set or show.
// the spec comes from the state when this window has it, so the node is drawn filled in on its first frame
function SpecNode({ nodeType, data, headerType }: { nodeType: string; data: any; headerType?: string }) {
    const fromState = useContext(StateContext)?.backEndState?.default_nodes?.nodes?.find((node: any) => node.id === nodeType);
    const [loaded, setLoaded] = useState<any | null>(null);

    useEffect(() => {
        if (!fromState) getNodeData(nodeType).then(setLoaded);
    }, [nodeType, fromState]);

    return <BaseNode nodeData={fromState ?? loaded} data={data} headerType={headerType} />;
}

export default SpecNode;
