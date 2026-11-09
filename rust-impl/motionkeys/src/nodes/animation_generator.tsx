import { useCallback, useEffect, useRef, useState } from "react";
import { message, open } from "@tauri-apps/plugin-dialog";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { useNodeSpec } from "../utils/nodeEntries";
import { useGraphOps, useSetInputs } from "../utils/graphOps";
import { invoke } from "@tauri-apps/api/core";
import CurvePreview from "../components/graph/CurvePreview";
import { useStore } from "@xyflow/react";
import ExpressionField from "../components/ExpressionField";
import { mapperText, useMapperVariables } from "../utils/mapper";
import { ValueSlider } from "../components/ValueSlider";
import { useGroupContext } from "../contexts/GroupContext";

// the animation overlap modes, same order and ids as ANIMATION_OVERLAPS in the backend
export const overlapModes = [
    { id: "add", name: "Add" },
    { id: "min", name: "Min" },
    { id: "max", name: "Max" },
    { id: "prev", name: "Previous" },
    { id: "next", name: "Next" },
    { id: "rvc", name: "Rest Value Crossing" },
    { id: "prune", name: "Keyframe Pruning" },
    { id: "crossfade", name: "Crossfade" },
    { id: "momentum", name: "Momentum" },
];

// a slider drag: its undo step, the value waiting to be sent, the last one sent and whether it's still on its way
type Drag = { txn: string; pending: number | null; sent: Promise<unknown>; sending: boolean };

// a number box for an input, set when it's left or on enter. empty unsets it so the backend default is used, anything
// that isn't a number is put back
function NumberField({ value, onCommit, ...props }: { value: number | undefined; onCommit: (value: number | null) => void } & React.InputHTMLAttributes<HTMLInputElement>) {
    const [text, setText] = useState<string | number>(value ?? "");
    useEffect(() => setText(value ?? ""), [value]);
    const commit = (e: React.FocusEvent<HTMLInputElement>) => {
        // a number half typed ("-", "1e", ".") reads as empty, it's put back instead of unsetting
        if (e.currentTarget.validity.badInput) {
            e.currentTarget.value = String(value ?? "");
            setText(value ?? "");
        } else if (text === "") {
            onCommit(null);
        } else if (!isNaN(Number(text))) {
            onCommit(Number(text));
        } else {
            setText(value ?? "");
        }
    };
    return (
        <input
            type="number"
            className="node-field nodrag border border-gray-400 rounded px-2 py-1"
            {...props}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
            }}
        />
    );
}

// a slider for an input from 0 to 1. a drag is one undo step, it sends its value at most once a frame and shows where
// it's dragged to, not what comes back from the backend on the way
function useSliderInput(id: string, input: string, current: number | undefined) {
    const setInputs = useSetInputs();
    const { scopeId } = useGroupContext();
    const { end } = useGraphOps(scopeId);
    const [value, setValue] = useState<number>(current ?? 0);
    const dragRef = useRef<Drag | null>(null);
    useEffect(() => {
        if (!dragRef.current) setValue(current ?? 0);
    }, [current]);

    // send the value waiting, in the drag's undo step. one send at a time, the latest value goes out when the one before
    // it lands, so a slow backend doesn't queue up a send per frame
    const flush = (drag: Drag): Promise<unknown> => {
        if (drag.pending === null || drag.sending) return drag.sent;
        const pending = drag.pending;
        drag.pending = null;
        drag.sending = true;
        drag.sent = setInputs(id, { [input]: pending }, { txn: drag.txn }).finally(() => {
            drag.sending = false;
            // only a value that came meanwhile, `drag.sent` is this promise itself and waiting on it never ends
            return drag.pending !== null ? flush(drag) : undefined;
        });
        return drag.sent;
    };
    const onChange = (next: number) => {
        setValue(next);
        const drag = dragRef.current;
        // typed in, its own undo step
        if (!drag) {
            setInputs(id, { [input]: next });
            return;
        }
        if (drag.pending === null) requestAnimationFrame(() => flush(drag));
        drag.pending = next;
    };
    const onStart = () => (dragRef.current = { txn: crypto.randomUUID(), pending: null, sent: Promise.resolve(), sending: false });
    // the last value goes out, then the undo step closes
    const onEnd = () => {
        const drag = dragRef.current;
        dragRef.current = null;
        if (!drag) return;
        flush(drag);
        drag.sent.finally(() => end(drag.txn));
    };
    return { value, onChange, onStart, onEnd };
}

function animation_generator({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const nodeData = useNodeSpec("animation_generator");
    const [name, setName] = useState(data.inputs?.name || ""); 

    useEffect(() => {
        setName(data.inputs?.name || "");
    }, [data.inputs?.name]);

    // only a change is sent, emptied unsets it
    const handleUpdate = useCallback(() => {
        if (name !== (data.inputs?.name || "")) setInputs(id, { name: name === "" ? null : name });
    }, [id, name, data.inputs?.name, setInputs]);

    const nameComponent = (
        <>
            <div>
                <input type="text" className="node-field border border-gray-400 rounded px-2 py-1" placeholder="Name" value={name} onChange={(e) => setName(e.target.value)} onBlur={handleUpdate} />
            </div>
        </>
    );

    // unset is the default, add
    const overlapComponent = (
        <>
            <div className="node-field field-inputs">
                <span>Animation Overlap</span>
            </div>
            <select className="node-field nodrag nopan" value={data.inputs?.animation_overlap || "add"} onChange={(e) => setInputs(id, { animation_overlap: e.target.value })}>
                {overlapModes.map((mode) => (
                    <option key={mode.id} value={mode.id}>
                        {mode.name}
                    </option>
                ))}
            </select>
        </>
    );

    // only crossfade uses the blend time
    const blendComponent =
        data.inputs?.animation_overlap === "crossfade" ? (
            <>
                <div className="node-field field-inputs">
                    <span>Blend (s)</span>
                </div>
                <NumberField min="0" step="0.05" placeholder="0.1" value={data.inputs?.overlap_blend} onCommit={(value) => setInputs(id, { overlap_blend: value })} />
            </>
        ) : (
            <></>
        );

    // a mapper is typed in unless something (a Note Table) is connected to it. stored edges are reversed, `source` is the
    // node taking the value
    const vars = useMapperVariables();
    const connected = useStore((s) => s.edges.flatMap((e) => (e.source === id ? [e.sourceHandle] : [])).join("\n")).split("\n");
    const mapperComponent = (input: "time_mapper" | "amplitude_mapper") =>
        connected.includes(input) ? (
            <></>
        ) : (
            <div style={{ position: "relative" }}>
                <ExpressionField className="node-field border border-gray-400 rounded px-2 py-1" value={mapperText(data.inputs?.[input])} vars={vars} onCommit={(text) => setInputs(id, { [input]: text === "" ? null : text })} />
            </div>
        );

    // an anchor point is typed in unless something is connected to it, in frames from the note on or off
    const anchorComponent = (input: "note_on_anchor_point" | "note_off_anchor_point") => (connected.includes(input) ? <></> : <NumberField step="1" placeholder="0" value={data.inputs?.[input]} onCommit={(value) => setInputs(id, { [input]: value })} />);

    // velocity intensity from 0 to 1
    const intensity = useSliderInput(id, "velocity_intensity", data.inputs?.velocity_intensity);
    const intensityComponent = connected.includes("velocity_intensity") ? <></> : <ValueSlider className="node-field nodrag nopan border border-gray-400 rounded" {...intensity} />;

    // only momentum uses dampen and use anchor point value. unset is on, the spec's default
    const momentum = data.inputs?.animation_overlap === "momentum";
    const dampen = useSliderInput(id, "overlap_dampen", data.inputs?.overlap_dampen);
    const dampenComponent = momentum ? (
        <>
            <div className="node-field field-inputs">
                <span>Dampen</span>
            </div>
            <ValueSlider className="node-field nodrag nopan border border-gray-400 rounded" {...dampen} />
        </>
    ) : (
        <></>
    );
    const anchorValueComponent = momentum ? (
        <label className="node-field flex items-center gap-1">
            <input type="checkbox" checked={data.inputs?.overlap_use_anchor_value ?? true} onChange={(e) => setInputs(id, { overlap_use_anchor_value: e.target.checked })} />
            Use Anchor Point Value
        </label>
    ) : (
        <></>
    );

    const uiInject = {
        name: nameComponent,
        note_on_anchor_point: anchorComponent("note_on_anchor_point"),
        note_off_anchor_point: anchorComponent("note_off_anchor_point"),
        time_mapper: mapperComponent("time_mapper"),
        amplitude_mapper: mapperComponent("amplitude_mapper"),
        velocity_intensity: intensityComponent,
        animation_overlap: overlapComponent,
        overlap_blend: blendComponent,
        overlap_dampen: dampenComponent,
        overlap_use_anchor_value: anchorValueComponent,
    };

    const hiddenHandles = {
        animation_overlap: true,
        overlap_blend: true,
        overlap_dampen: true,
        overlap_use_anchor_value: true,
    };

    // the generator's curves from the last run, under the inputs
    return (
        <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data}>
            <CurvePreview outputs={state?.executed_results?.[id]} />
        </BaseNode>
    );
}

export default animation_generator;
