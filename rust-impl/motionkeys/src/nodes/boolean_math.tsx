import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { CheckboxInput, OperationSelect, useConnectedInputs } from "../components/nodegraph/NodeFields";

// the operations, same order and ids as BOOLEAN_OPERATIONS in src-tauri/src/graph/executors/primitives.rs
export const booleanOperations = [
    { id: "and", name: "And" },
    { id: "or", name: "Or" },
    { id: "not", name: "Not" },
    { id: "nand", name: "Not And" },
    { id: "nor", name: "Nor" },
    { id: "equal", name: "Equal" },
    { id: "not_equal", name: "Not Equal" },
];

function boolean_math({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const nodeData = useNodeSpec("boolean_math");
    const connected = useConnectedInputs(id);
    const not = (data?.inputs?.operation ?? "and") === "not";

    // a value is ticked on the node, next to its socket, unless something is connected to it. the second one goes away
    // for not, unless it's connected so its link stays drawn
    const checkbox = (input: string) => (connected.includes(input) ? undefined : <CheckboxInput id={id} input={input} label="Boolean" data={data} nodeData={nodeData} />);
    const hideB = not && !connected.includes("b");

    return <BaseNode nodeData={nodeData} inject={{ operation: <OperationSelect id={id} data={data} nodeData={nodeData} operations={booleanOperations} /> }} labels={{ a: checkbox("a"), b: hideB ? undefined : checkbox("b") }} hidden={{ operation: true, b: hideB }} hideValues={{ a: true, b: true }} data={data} />;
}

export default boolean_math;
