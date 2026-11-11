import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { NumberField, useSliderInput } from "./animation_generator";
import { useStateContext } from "../contexts/StateContext";
import { useNodeSpec } from "../utils/nodeEntries";
import { useSetInputs } from "../utils/graphOps";
import CurvePreview from "../components/graph/CurvePreview";
import { ValueSlider } from "../components/ValueSlider";
import { CurveChannel } from "../utils/curves";

// the shapes, same order and ids as PRESET_ANIMATION_MODES in the backend
export const modes = [
    { id: "spring", name: "Spring" },
    { id: "drumstick", name: "Drumstick" },
    { id: "cymbal", name: "Cymbal" },
];

// every setting, set on the node only. each mode shows its own
const SETTINGS = ["start", "end", "period", "amplitude", "dampen", "flip", "rest", "angle", "detune", "rest_y", "windup", "windup_value", "hit", "hit_value", "recoil", "recoil_value", "rest_length"];

// a text box for a channel, sent when it's left. emptied, the node puts the default back
function ChannelField({ value, onCommit }: { value: string | undefined; onCommit: (value: string | null) => void }) {
    const [text, setText] = useState(value ?? "");
    useEffect(() => setText(value ?? ""), [value]);
    const commit = () => {
        if (text !== (value ?? "")) onCommit(text === "" ? null : text);
    };
    return (
        <div>
            <input type="text" className="node-field nodrag border border-gray-400 rounded px-2 py-1" value={text} onChange={(e) => setText(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
        </div>
    );
}

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

    // a small number box, frames and degrees step by whole ones. emptied, it goes back to the default, which is kept on
    // the node like every other value (the spec's store_defaults)
    const box = (input: string, step = "0.05") => <NumberField className="nodrag border border-gray-400 rounded preset-box" step={step} placeholder={String(fallback(input) ?? "")} value={data.inputs?.[input]} onDraft={(value) => draft.set(input, value)} onCommit={(value) => setInputs(id, { [input]: value ?? fallback(input) ?? null })} />;
    const row = (name: string, ...boxes: JSX.Element[]) => (
        <div className="node-field preset-row" key={name}>
            <span>{name}</span>
            {boxes.map((box, i) => (
                <Fragment key={i}>{box}</Fragment>
            ))}
        </div>
    );
    const slider = (name: string, props: typeof dampen) => <ValueSlider key={name} label={name} className="node-field nodrag nopan border border-gray-400 rounded preset-slider" {...props} />;

    // a drumstick's phases, how many frames each takes and the value it ends at. it starts at rest, the hit lands on the
    // note
    const drumstickRows = [
        <div className="node-field preset-row preset-columns" key="columns">
            <span></span>
            <span>Frames</span>
            <span>Value</span>
        </div>,
        row("Windup", box("windup", "1"), box("windup_value")),
        row("Hit", box("hit", "1"), box("hit_value")),
        row("Recoil", box("recoil", "1"), box("recoil_value")),
        row("Rest", box("rest_length", "1"), box("rest")),
    ];
    const springRows = [row("Start", box("start", "1")), row("End", box("end", "1")), row("Period", box("period", "1")), row("Amplitude", box("amplitude")), slider("Dampen", dampen)];
    const flipRow = (
        <label className="node-field flex items-center gap-1" key="flip">
            <input type="checkbox" checked={data.inputs?.flip ?? false} onChange={(e) => setInputs(id, { flip: e.target.checked })} />
            Flip
        </label>
    );
    const rows: Record<string, JSX.Element[]> = {
        spring: [...springRows, row("Rest", box("rest")), flipRow],
        drumstick: drumstickRows,
        cymbal: [...springRows, row("Angle", box("angle", "1")), row("Detune", box("detune", "0.1")), row("Rest", box("rest"), box("rest_y"))],
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
    const channel = (input: "channel" | "channel_y") => <ChannelField value={data.inputs?.[input] ?? spec(input)?.default} onCommit={(value) => setInputs(id, { [input]: value ?? spec(input)?.default ?? null })} />;

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
        <BaseNode nodeData={nodeData} inject={inject} hidden={hidden} labels={labels} data={data}>
            <CurvePreview outputs={results} curves={draft.curves} />
        </BaseNode>
    );
}

export default preset_animation;
