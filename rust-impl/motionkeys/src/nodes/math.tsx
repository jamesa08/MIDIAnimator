import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { CheckboxInput, OperationSelect, useConnectedInputs } from "../components/nodegraph/NodeFields";

// the operations, same order and ids as MATH_OPERATIONS in src-tauri/src/graph/executors/primitives.rs
export const mathOperations = [
    { id: "add", name: "Add" },
    { id: "subtract", name: "Subtract" },
    { id: "multiply", name: "Multiply" },
    { id: "divide", name: "Divide" },
    { id: "power", name: "Power" },
    { id: "minimum", name: "Minimum" },
    { id: "maximum", name: "Maximum" },
    { id: "modulo", name: "Truncated Modulo" },
    { id: "floored_modulo", name: "Floored Modulo" },
    { id: "absolute", name: "Absolute" },
    { id: "round", name: "Round" },
    { id: "floor", name: "Floor" },
    { id: "ceil", name: "Ceil" },
    { id: "sine", name: "Sine" },
    { id: "cosine", name: "Cosine" },
];

// the operations on the first value alone
export const UNARY_MATH = new Set(["absolute", "round", "floor", "ceil", "sine", "cosine"]);

function math({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const nodeData = useNodeSpec("math");
    const connected = useConnectedInputs(id);
    const unary = UNARY_MATH.has(data?.inputs?.operation ?? "add");

    // the second value goes away for an operation that doesn't use it, unless it's connected so its link stays drawn
    const hideB = unary && !connected.includes("b");

    const inject = {
        operation: <OperationSelect id={id} data={data} nodeData={nodeData} operations={mathOperations} />,
        clamp: <CheckboxInput id={id} input="clamp" label="Clamp" data={data} nodeData={nodeData} />,
    };
    return <BaseNode nodeData={nodeData} inject={inject} hidden={{ operation: true, clamp: true, b: hideB }} data={data} />;
}

export default math;
