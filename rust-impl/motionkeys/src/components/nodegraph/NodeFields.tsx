import { useCallback, useRef } from "react";
import { CLICK_DISTANCE } from "./canvasInput";
import { useStore } from "@xyflow/react";
import { NumberBox } from "../NumberBox";
import { useSetInputs } from "../../utils/graphOps";

// fields on nodes, like Blender's: a value is typed in on the node unless something is connected to its input. a
// number socket's box is BaseNode's own

// the inputs of node `id` something is connected to. stored edges are reversed, `source`/`sourceHandle` is the node
// taking the value and its input
export function useConnectedInputs(id: string): string[] {
    const select = useCallback((s: any) => s.edges.flatMap((e: any) => (e.source === id ? [e.sourceHandle] : [])).join("\n"), [id]);
    return useStore(select).split("\n");
}

// what an input runs with when it isn't set, its spec's default
export const specDefault = (nodeData: any, input: string) => nodeData?.handles?.inputs?.find((h: any) => h.id === input)?.default;

// the box for a number input with no socket, on a row of its own, showing what it runs with. emptied unsets it, back to
// the default
export function NumberInput({ id, input, label, data, nodeData }: { id: string; input: string; label: string; data: any; nodeData: any }) {
    const setInputs = useSetInputs();
    return <NumberBox className="node-field border border-gray-400 rounded" label={label} value={data?.inputs?.[input] ?? specDefault(nodeData, input)} readOnly={data === "preview"} onChange={(value) => setInputs(id, { [input]: value })} onClear={() => setInputs(id, { [input]: null })} />;
}

// a checkbox with its name, every one on a node. the box itself is nodrag, its name drags the node like the rest of it:
// a click on the name ticks it, a press that moved (a drag of the node) doesn't
export function CheckboxRow({ label, checked, onChange }: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
    const press = useRef<{ x: number; y: number } | null>(null);
    return (
        <label
            className="node-field flex items-center gap-1"
            onPointerDown={(e) => (press.current = { x: e.clientX, y: e.clientY })}
            onClick={(e) => {
                const from = press.current;
                press.current = null;
                if (from && Math.hypot(e.clientX - from.x, e.clientY - from.y) > CLICK_DISTANCE) e.preventDefault();
            }}
        >
            <input type="checkbox" className="nodrag" checked={checked} onChange={(e) => onChange(e.target.checked)} />
            {label}
        </label>
    );
}

// a checkbox for a bool input, with its name
export function CheckboxInput({ id, input, label, data, nodeData }: { id: string; input: string; label: string; data: any; nodeData: any }) {
    const setInputs = useSetInputs();
    const checked = data?.inputs?.[input] ?? specDefault(nodeData, input) ?? false;
    return <CheckboxRow label={label} checked={checked} onChange={(value) => setInputs(id, { [input]: value })} />;
}

// the dropdown for a node's operation, from its list of { id, name }
export function OperationSelect({ id, data, nodeData, operations }: { id: string; data: any; nodeData: any; operations: { id: string; name: string }[] }) {
    const setInputs = useSetInputs();
    return (
        <select className="node-field nodrag nopan" value={data?.inputs?.operation ?? specDefault(nodeData, "operation")} onChange={(e) => setInputs(id, { operation: e.target.value })}>
            {operations.map((operation) => (
                <option key={operation.id} value={operation.id}>
                    {operation.name}
                </option>
            ))}
        </select>
    );
}
