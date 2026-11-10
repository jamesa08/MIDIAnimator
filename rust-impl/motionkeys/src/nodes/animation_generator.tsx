import { useCallback, useEffect, useState } from "react";
import { message, open } from "@tauri-apps/plugin-dialog";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { useNodeSpec } from "../utils/nodeEntries";
import { useInputDrag, useSetInputs } from "../utils/graphOps";
import { invoke } from "@tauri-apps/api/core";
import CurvePreview from "../components/graph/CurvePreview";
import PhaseEditor from "../components/graph/PhaseEditor";
import { useStore } from "@xyflow/react";
import ExpressionField from "../components/ExpressionField";
import { mapperText, useMapperVariables } from "../utils/mapper";
import { ValueSlider } from "../components/ValueSlider";

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
    const drag = useInputDrag(id);
    const [value, setValue] = useState<number>(current ?? 0);
    useEffect(() => {
        if (!drag.dragging()) setValue(current ?? 0);
    }, [current]);
    const onChange = (next: number) => {
        setValue(next);
        drag.send({ [input]: next });
    };
    return { value, onChange, onStart: drag.start, onEnd: drag.stop };
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

    // momentum's windup, hit and recoil on the note on curve, once a run has found them
    const generator = state?.executed_results?.[id]?.generator;
    const sceneFps = (state?.scene as any)?.fps;
    const fps: number = typeof sceneFps === "number" && sceneFps > 0 ? sceneFps : 24;
    const phaseComponent = momentum && generator?.overlap_windup != null && generator?.overlap_recoil != null ? <PhaseEditor id={id} generator={generator} fps={fps} anchorConnected={connected.includes("note_on_anchor_point")} set={{ windup: data.inputs?.overlap_windup != null, recoil: data.inputs?.overlap_recoil != null }} /> : <></>;

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
        overlap_windup: phaseComponent,
        overlap_recoil: <></>,
    };

    const hiddenHandles = {
        animation_overlap: true,
        overlap_blend: true,
        overlap_dampen: true,
        overlap_use_anchor_value: true,
        overlap_windup: true,
        overlap_recoil: true,
    };

    // the generator's curves from the last run, under the inputs
    return (
        <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data}>
            <CurvePreview outputs={state?.executed_results?.[id]} />
        </BaseNode>
    );
}

export default animation_generator;
