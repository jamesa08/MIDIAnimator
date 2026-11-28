import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { OperationSelect, useConnectedInputs } from "../components/nodegraph/NodeFields";

// the comparisons, same order and ids as COMPARE_OPERATIONS in src-tauri/src/graph/executors/primitives.rs
export const compareOperations = [
    { id: "less_than", name: "Less Than" },
    { id: "less_equal", name: "Less Than or Equal" },
    { id: "greater_than", name: "Greater Than" },
    { id: "greater_equal", name: "Greater Than or Equal" },
    { id: "equal", name: "Equal" },
    { id: "not_equal", name: "Not Equal" },
];

// the comparisons that use epsilon
export const EPSILON_COMPARES = new Set(["equal", "not_equal"]);

function compare({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const nodeData = useNodeSpec("compare");
    const connected = useConnectedInputs(id);

    // epsilon is only there for equal and not equal, unless it's connected so its link stays drawn
    const hideEpsilon = !EPSILON_COMPARES.has(data?.inputs?.operation ?? "greater_than") && !connected.includes("epsilon");

    return <BaseNode nodeData={nodeData} inject={{ operation: <OperationSelect id={id} data={data} nodeData={nodeData} operations={compareOperations} /> }} hidden={{ operation: true, epsilon: hideEpsilon }} data={data} />;
}

export default compare;
