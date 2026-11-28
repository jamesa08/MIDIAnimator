import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { CheckboxRow } from "../components/nodegraph/NodeFields";
import { useSliderInput } from "./animation_generator";
import { useStateContext } from "../contexts/StateContext";
import { useNodeSpec } from "../utils/node";
import { useSetInputs } from "../utils/graphOps";
import CurvePreview from "../components/graph/CurvePreview";
import { TILE_PICTURE } from "../components/nodegraph/simplified";
import { NumberBox } from "../components/NumberBox";
import { TextBox } from "../components/TextBox";
import { CurveChannel } from "../utils/curves";

// the shapes, same order and ids as PRESET_ANIMATION_MODES in the backend
const modes = [
    { id: "spring", name: "Spring" },
    { id: "drumstick", name: "Drumstick" },
    { id: "cymbal", name: "Cymbal" },
];

// every setting, set on the node only. each mode shows its own
const SETTINGS = ["start", "end", "period", "amplitude", "dampen", "flip", "rest", "angle", "detune", "rest_y", "windup", "windup_value", "hit", "hit_value", "recoil", "recoil_value", "rest_length"];

// values being dragged or typed, drawn on the preview before they're set: only the node's own function runs on them
// (graph_node_preview), the graph runs once when they're set. new results or inputs (the value set, an undo) end the
// draft. one request at a time, the latest values go next
function useDraftCurves(nodeType: string, inputs: any, results: any) {
    const [draft, setDraft] = useState<Record<string, any> | null>(null);
    const [curves, setCurves] = useState<CurveChannel[] | null>(null);
    const resultsKey = useMemo(() => JSON.stringify(results ?? null), [results]);
    const inputsKey = useMemo(() => JSON.stringify(inputs ?? null), [inputs]);
    useEffect(() => setDraft(null), [resultsKey, inputsKey]);

    // which draft a request belongs to, an answer for one that's over is dropped
    const generation = useRef(0);
    const busy = useRef(false);
    const pending = useRef<Record<string, any> | null>(null);
    useEffect(() => {
        generation.current += 1;
        if (!draft) {
            pending.current = null;
            setCurves(null);
            return;
        }
        pending.current = { ...inputs, ...draft };
        if (busy.current) return;
        const send = () => {
            const values = pending.current;
            pending.current = null;
            busy.current = values != null;
            if (!values) return;
            const asked = generation.current;
            invoke<CurveChannel[]>("graph_node_preview", { nodeType, inputs: values })
                .then((next) => asked === generation.current && setCurves(next))
                // a value half typed (an end before its start) keeps the last preview
                .catch(() => {})
                .finally(send);
        };
        send();
    }, [draft]);

    // a value of `input` that isn't set yet, null takes it out of the draft
    const set = (input: string, value: any) =>
        setDraft((current) => {
            const next = { ...(current ?? {}) };
            if (value == null) delete next[input];
            else next[input] = value;
            return Object.keys(next).length ? next : null;
        });
    return { curves, set };
}

function preset_animation({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const { backEndState: state } = useStateContext();
    const nodeData = useNodeSpec("preset_animation");
    const mode: string = data.inputs?.mode || "spring";
    const cymbal = mode === "cymbal";
    const spec = (input: string) => nodeData?.handles?.inputs?.find((h: any) => h.id === input);
    // what an unset setting is: the spec's default, or the mode's for the ones spring and cymbal start differently
    const fallback = (input: string) => spec(input)?.default ?? spec(input)?.mode_defaults?.[mode];

    const results = state?.executed_results?.[id];
    const draft = useDraftCurves("preset_animation", data.inputs, results);
    const dampenInput = useSliderInput(id, "dampen", data.inputs?.dampen ?? fallback("dampen"));
    const dampen = {
        ...dampenInput,
        onChange: (value: number) => {
            dampenInput.onChange(value);
            draft.set("dampen", value);
        },
    };

    // a setting's number box, the curve redrawn as it's typed. emptied, it goes back to the default, which is kept on the
    // node like every other value (the spec's store_defaults). `label` puts its name in it on a row of its own, without
    // one it's a cell of a row
    const box = (input: string, label?: string) => (
        <NumberBox key={input} label={label} className={`border border-gray-400 rounded ${label ? "node-field" : "preset-box"}`} value={data.inputs?.[input] ?? fallback(input)} readOnly={data === "preview"} onDraft={(value) => draft.set(input, value)} onChange={(value) => setInputs(id, { [input]: value })} onClear={() => setInputs(id, { [input]: fallback(input) ?? null })} />
    );
    const row = (name: string, ...boxes: JSX.Element[]) => (
        <div className="node-field preset-row" key={name}>
            <span>{name}</span>
            {boxes.map((box, i) => (
                <Fragment key={i}>{box}</Fragment>
            ))}
        </div>
    );
    const slider = (name: string, props: typeof dampen) => <NumberBox slider key={name} label={name} className="node-field border border-gray-400 rounded" {...props} />;

    // a drumstick's phases, how many frames each takes and the value it ends at. it starts at rest, the hit lands on the
    // note
    const drumstickRows = [
        <div className="node-field preset-row preset-columns" key="columns">
            <span></span>
            <span>Frames</span>
            <span>Value</span>
        </div>,
        row("Windup", box("windup"), box("windup_value")),
        row("Hit", box("hit"), box("hit_value")),
        row("Recoil", box("recoil"), box("recoil_value")),
        row("Rest", box("rest_length"), box("rest")),
    ];
    const springRows = [box("start", "Start"), box("end", "End"), box("period", "Period"), box("amplitude", "Amplitude"), slider("Dampen", dampen)];
    const flipRow = <CheckboxRow key="flip" label="Flip" checked={data.inputs?.flip ?? false} onChange={(checked) => setInputs(id, { flip: checked })} />;
    const rows: Record<string, JSX.Element[]> = {
        spring: [...springRows, box("rest", "Rest"), flipRow],
        drumstick: drumstickRows,
        cymbal: [...springRows, box("angle", "Angle"), box("detune", "Detune"), row("Rest", box("rest"), box("rest_y"))],
    };

    const modeComponent = (
        <>
            <select className="node-field nodrag nopan" value={mode} onChange={(e) => setInputs(id, { mode: e.target.value })}>
                {modes.map((m) => (
                    <option key={m.id} value={m.id}>
                        {m.name}
                    </option>
                ))}
            </select>
            {rows[mode] ?? rows.spring}
        </>
    );

    // each curve's channel under its output
    const channel = (input: "channel" | "channel_y") => <TextBox className="node-field" label="Channel" value={data.inputs?.[input] ?? spec(input)?.default} readOnly={data === "preview"} onCommit={(text) => setInputs(id, { [input]: text === "" ? (spec(input)?.default ?? null) : text })} />;

    const inject: Record<string, JSX.Element> = {
        mode: modeComponent,
        keyframes: channel("channel"),
        keyframes_y: cymbal ? channel("channel_y") : <></>,
    };
    const hidden: Record<string, boolean> = { mode: true, channel: true, channel_y: true, keyframes_y: !cymbal };
    for (const input of SETTINGS) {
        inject[input] = <></>;
        hidden[input] = true;
    }

    // a cymbal's first curve is its X, next to its Y
    const labels = cymbal ? { keyframes: <span style={{ float: "right", marginLeft: "auto" }}>Keyframes X</span> } : {};

    // the curves from the last run under the settings, or the draft's while a value is dragged or typed
    return (
        <BaseNode nodeData={nodeData} inject={inject} hidden={hidden} labels={labels} data={data} tile={<CurvePreview outputs={results} curves={draft.curves} height={TILE_PICTURE} />}>
            <CurvePreview outputs={results} curves={draft.curves} />
        </BaseNode>
    );
}

export default preset_animation;
