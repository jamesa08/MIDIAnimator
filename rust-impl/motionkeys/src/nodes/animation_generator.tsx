import { useEffect, useState } from "react";
import { message, open } from "@tauri-apps/plugin-dialog";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { useNodeSpec } from "../utils/node";
import { useInputDrag, useSetInputs } from "../utils/graphOps";
import { invoke } from "@tauri-apps/api/core";
import CurvePreview from "../components/graph/CurvePreview";
import { TILE_PICTURE } from "../components/nodegraph/simplified";
import PhaseEditor from "../components/graph/PhaseEditor";
import { useStore } from "@xyflow/react";
import ExpressionField from "../components/ExpressionField";
import { mapperText, useMapperVariables } from "../utils/mapper";
import { NumberBox } from "../components/NumberBox";
import { TextBox } from "../components/TextBox";
import { CheckboxRow, NumberInput } from "../components/nodegraph/NodeFields";

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

// a slider for an input from 0 to 1. a drag is one undo step, it sends its value at most once a frame and shows where
// it's dragged to, not what comes back from the backend on the way
export function useSliderInput(id: string, input: string, current: number | undefined) {
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
    const blendComponent = data.inputs?.animation_overlap === "crossfade" ? <NumberInput id={id} input="overlap_blend" label="Blend (s)" data={data} nodeData={nodeData} /> : <></>;

    // the name and the mappers are typed in beside their sockets unless something (a Note Table) is connected to them,
    // emptied unsets them. stored edges are reversed, `source` is the node taking the value
    const vars = useMapperVariables();
    const connected = useStore((s) => s.edges.flatMap((e) => (e.source === id ? [e.sourceHandle] : [])).join("\n")).split("\n");
    const nameComponent = connected.includes("name") ? undefined : <TextBox label="Name" value={data.inputs?.name ?? ""} readOnly={data === "preview"} onCommit={(text) => setInputs(id, { name: text === "" ? null : text })} />;
    const mapperComponent = (input: "time_mapper" | "amplitude_mapper", label: string) =>
        connected.includes(input) ? undefined : (
            <TextBox label={label}>
                <ExpressionField className="text-box-input" value={mapperText(data.inputs?.[input])} vars={vars} onCommit={(text) => setInputs(id, { [input]: text === "" ? null : text })} />
            </TextBox>
        );

    // velocity intensity from 0 to 1, a slider beside its socket unless something is connected to it. the anchor points
    // are BaseNode's number boxes
    const intensity = useSliderInput(id, "velocity_intensity", data.inputs?.velocity_intensity);
    const intensityComponent = connected.includes("velocity_intensity") ? undefined : <NumberBox slider label="Velocity Intensity" className="border border-gray-400 rounded" {...intensity} />;

    // only momentum uses dampen and use anchor point value. unset is on, the spec's default
    const momentum = data.inputs?.animation_overlap === "momentum";
    const dampen = useSliderInput(id, "overlap_dampen", data.inputs?.overlap_dampen);
    const dampenComponent = momentum ? <NumberBox slider label="Dampen" className="node-field border border-gray-400 rounded" {...dampen} /> : <></>;
    const anchorValueComponent = momentum ? (
        <CheckboxRow label="Use Anchor Point Value" checked={data.inputs?.overlap_use_anchor_value ?? true} onChange={(checked) => setInputs(id, { overlap_use_anchor_value: checked })} />
    ) : (
        <></>
    );

    // momentum's windup, hit and recoil on the note on curve, once a run has found them
    const generator = state?.executed_results?.[id]?.generator;
    const sceneFps = (state?.scene as any)?.fps;
    const fps: number = typeof sceneFps === "number" && sceneFps > 0 ? sceneFps : 24;
    const phaseComponent = momentum && generator?.overlap_windup != null && generator?.overlap_recoil != null ? <PhaseEditor id={id} generator={generator} fps={fps} anchorConnected={connected.includes("note_on_anchor_point")} set={{ windup: data.inputs?.overlap_windup != null, recoil: data.inputs?.overlap_recoil != null }} /> : <></>;

    const uiInject = {
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
        <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} labels={{ name: nameComponent, time_mapper: mapperComponent("time_mapper", "Time Mapper"), amplitude_mapper: mapperComponent("amplitude_mapper", "Amplitude Mapper"), velocity_intensity: intensityComponent }} hideValues={{ velocity_intensity: true }} data={data} tile={<CurvePreview outputs={state?.executed_results?.[id]} height={TILE_PICTURE} />}>
            <CurvePreview outputs={state?.executed_results?.[id]} />
        </BaseNode>
    );
}

export default animation_generator;
