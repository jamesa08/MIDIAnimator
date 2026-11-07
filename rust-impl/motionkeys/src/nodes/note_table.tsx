import { useEffect, useState } from "react";
import { useStore } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { useNodeSpec } from "../utils/nodeEntries";
import { useSetInputs } from "../utils/graphOps";
import ExpressionField from "../components/ExpressionField";
import { noteToName } from "./assign_notes_to_objects";

// the curve presets, an expression in t (0 at the first note, 1 at the last). picking one fills in the curve, changing
// the curve picks the preset it matches or custom
export const SHAPES: { id: string; name: string; curve: string | null }[] = [
    { id: "constant", name: "Constant", curve: "" },
    { id: "linear", name: "Linear", curve: "t" },
    { id: "ease_in", name: "Ease In", curve: "t ** 2" },
    { id: "ease_out", name: "Ease Out", curve: "1 - (1 - t) ** 2" },
    { id: "smooth", name: "Smooth", curve: "t ** 2 * (3 - 2 * t)" },
    { id: "smoother", name: "Smoother", curve: "t ** 3 * (t * (6 * t - 15) + 10)" },
    { id: "cubic_in", name: "Cubic In", curve: "t ** 3" },
    { id: "cubic_out", name: "Cubic Out", curve: "1 - (1 - t) ** 3" },
    { id: "sine_in", name: "Sine In", curve: "1 - cos(t * pi / 2)" },
    { id: "sine_out", name: "Sine Out", curve: "sin(t * pi / 2)" },
    { id: "sine_in_out", name: "Sine In Out", curve: "(1 - cos(t * pi)) / 2" },
    { id: "exponential", name: "Exponential", curve: "(2 ** (10 * t) - 1) / 1023" },
    { id: "logarithmic", name: "Logarithmic", curve: "log(1 + 9 * t, 10)" },
    { id: "square_root", name: "Square Root", curve: "sqrt(t)" },
    { id: "steps", name: "Steps", curve: "floor(t * 4) / 4" },
    { id: "custom", name: "Custom", curve: null },
];

// the names a curve can use (CURVE_VARS in src-tauri/src/utils/note_table.rs)
const CURVE_VARS = ["t", "note"];

// the table's settings as the node saves them, with the backend's defaults (NoteTable in note_table.rs)
type Table = { target: string; operation: string; source: string; from: number; to: number; shape: string; curve: string; min: number; max: number; overrides: { note: number; value: number }[]; fallback: string; fallback_value: number };
const DEFAULTS: Table = { target: "amplitude", operation: "multiply", source: "map", from: 36, to: 47, shape: "linear", curve: "t", min: 0, max: 1, overrides: [], fallback: "keep", fallback_value: 0 };

// what the node ran with: a row per note and the curve between them
type Row = { note: number; objects: string[]; value: number; overridden: boolean };
type Preview = { rows: Row[]; curve: number[] };

// a number shown short, no float noise
const short = (value: number) => String(Math.round(value * 1e4) / 1e4);

// a number box that's set when it's left or on enter, escape or anything that isn't a number puts it back
function NumberBox({ value, onCommit, width, className, readOnly, integer }: { value: number; onCommit: (value: number) => void; width: number; className?: string; readOnly?: boolean; integer?: boolean }) {
    const [text, setText] = useState(short(value));
    useEffect(() => setText(short(value)), [value]);
    const commit = () => {
        const n = Number(text);
        if (text.trim() === "" || !Number.isFinite(n) || (integer && !Number.isInteger(n))) setText(short(value));
        else if (n !== value) onCommit(n);
    };
    return (
        <input
            className={`note-table-input nodrag ${className ?? ""}`}
            style={{ width }}
            value={text}
            readOnly={readOnly}
            onChange={(e) => setText(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
                if (e.key === "Escape") {
                    setText(short(value));
                    requestAnimationFrame(() => (e.target as HTMLInputElement).blur());
                }
            }}
        />
    );
}

// the bars of the rows' values, the selected one darker, and the curve dashed over them
function PreviewBars({ preview, selected, onSelect }: { preview: Preview; selected: number | null; onSelect: (note: number) => void }) {
    const [width, height] = [260, 64];
    const rows = preview.rows;
    if (rows.length === 0) return <div className="note-table-preview" style={{ height }} />;
    const values = [0, ...rows.map((r) => r.value), ...preview.curve];
    const [low, high] = [Math.min(...values), Math.max(...values)];
    const y = (v: number) => height - 4 - ((v - low) / (high - low || 1)) * (height - 8);
    const cell = width / rows.length;
    const gap = Math.min(2, cell * 0.15);
    const curve = preview.curve.map((v, i) => `${i ? "L" : "M"}${cell / 2 + (i / Math.max(preview.curve.length - 1, 1)) * (width - cell)},${y(v)}`).join("");
    return (
        <svg className="note-table-preview" width="100%" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" style={{ height }}>
            <line x1={0} x2={width} y1={y(0)} y2={y(0)} className="note-table-axis" />
            {rows.map((row, i) => (
                <rect key={row.note} x={i * cell + gap} y={Math.min(y(row.value), y(0))} width={Math.max(1, cell - 2 * gap)} height={Math.max(1, Math.abs(y(0) - y(row.value)))} className={`note-table-bar${row.overridden ? " overridden" : ""}${row.note === selected ? " selected" : ""}`} onClick={() => onSelect(row.note)} />
            ))}
            {rows.length > 1 && curve && <path d={curve} className="note-table-curve" vectorEffect="non-scaling-stroke" />}
        </svg>
    );
}

// a value per note for a mapper: a curve over the notes of the connected note map (spaced evenly) or a range, with values
// typed in over it. the backend works out the rows and the expression (src-tauri/src/utils/note_table.rs)
function note_table({ id, data }: { id: any; data: any }) {
    const setInputs = useSetInputs();
    const nodeData = useNodeSpec("note_table");
    const { backEndState: state } = useStateContext();
    const previewNode = data === "preview" || data?.preview === true;
    const [selected, setSelected] = useState<number | null>(null);

    const table: Table = { ...DEFAULTS, ...(previewNode ? {} : (data?.inputs?.table ?? {})) };
    const ran: Preview = state?.executed_results?.[id]?.preview ?? { rows: [], curve: [] };
    const preview: Preview = { rows: Array.isArray(ran.rows) ? ran.rows : [], curve: Array.isArray(ran.curve) ? ran.curve : [] };
    const constant = table.shape === "constant";
    // the note map is only a source while one is connected, without one the table is a range (like the backend runs it).
    // stored edges are reversed, `source` is the node taking the value
    const mapConnected = useStore((s) => s.edges.some((e) => e.source === id && e.sourceHandle === "note_map"));
    const source = mapConnected ? table.source : "range";

    // every change sets the whole table, one undo step
    const set = (changes: Partial<Table>) => setInputs(id, { table: { ...table, ...changes } });
    const setShape = (shape: string) => {
        const curve = SHAPES.find((s) => s.id === shape)?.curve;
        set({ shape, curve: curve ?? table.curve });
    };
    const setCurve = (curve: string) => set({ curve, shape: SHAPES.find((s) => s.curve === curve.trim() && s.id !== "constant")?.id ?? "custom" });
    const setOverride = (note: number, value: number | null) => set({ overrides: [...table.overrides.filter((o) => o.note !== note), ...(value === null ? [] : [{ note, value }])].sort((a, b) => a.note - b.note) });

    const controls = previewNode ? (
        <></>
    ) : (
        <div className="note-table">
            <div className="note-table-line">
                <select className="note-table-input nodrag" style={{ flex: 1 }} value={table.target} onChange={(e) => set({ target: e.target.value })}>
                    <option value="amplitude">Amplitude</option>
                    <option value="time">Time</option>
                </select>
                <select className="note-table-input nodrag" style={{ flex: 1 }} value={table.operation} onChange={(e) => set({ operation: e.target.value })}>
                    <option value="multiply">Multiply</option>
                    <option value="add">Add</option>
                    <option value="set">Set</option>
                </select>
            </div>
            <div className="note-table-line">
                <select className="note-table-input nodrag" style={{ flex: 1 }} value={source} onChange={(e) => set({ source: e.target.value })}>
                    {mapConnected && <option value="map">Connected Note Map</option>}
                    <option value="range">Range</option>
                </select>
                {source === "range" && (
                    <>
                        <NumberBox value={table.from} width={38} integer onCommit={(from) => set({ from: Math.max(0, Math.min(127, from)) })} />
                        <span>–</span>
                        <NumberBox value={table.to} width={38} integer onCommit={(to) => set({ to: Math.max(0, Math.min(127, to)) })} />
                    </>
                )}
            </div>
            <div className="note-table-line">
                <select className="note-table-input nodrag" style={{ width: 96 }} value={SHAPES.some((s) => s.id === table.shape) ? table.shape : "custom"} onChange={(e) => setShape(e.target.value)}>
                    {SHAPES.map((shape) => (
                        <option key={shape.id} value={shape.id}>
                            {shape.name}
                        </option>
                    ))}
                </select>
                {!constant && (
                    <div style={{ position: "relative", flex: 1, minWidth: 0, display: "flex" }}>
                        <ExpressionField className="note-table-input" style={{ width: "100%" }} value={table.curve} vars={CURVE_VARS} onCommit={setCurve} />
                    </div>
                )}
            </div>
            <div className="note-table-line">
                <span>{constant ? "Value" : "Min"}</span>
                <NumberBox value={table.min} width={52} onCommit={(min) => set({ min })} />
                {!constant && (
                    <>
                        <span style={{ marginLeft: 4 }}>Max</span>
                        <NumberBox value={table.max} width={52} onCommit={(max) => set({ max })} />
                    </>
                )}
            </div>
            <PreviewBars preview={preview} selected={selected} onSelect={setSelected} />
            <div className="note-table-rows nowheel">
                {preview.rows.map((row) => (
                    <div key={row.note} className={`note-table-row${row.note === selected ? " selected" : ""}`} onClick={() => setSelected(row.note)}>
                        <span className="note-table-note">{row.note}</span>
                        <span className="note-table-name">{noteToName(row.note)}</span>
                        <span className="note-table-objects">{row.objects.join(", ")}</span>
                        <NumberBox value={row.value} width={60} className={row.overridden ? "" : "computed"} onCommit={(value) => setOverride(row.note, value)} />
                        <button className="note-table-clear nodrag" style={{ visibility: row.overridden ? "visible" : "hidden" }} onClick={(e) => (e.stopPropagation(), setOverride(row.note, null))}>
                            ×
                        </button>
                    </div>
                ))}
            </div>
            <div className="note-table-line">
                <span>Fallback</span>
                <select className="note-table-input nodrag" style={{ flex: 1 }} value={table.fallback} onChange={(e) => set({ fallback: e.target.value })}>
                    <option value="keep">Keep</option>
                    <option value="zero">Zero</option>
                    <option value="value">Value</option>
                </select>
                {table.fallback === "value" && <NumberBox value={table.fallback_value} width={52} onCommit={(fallback_value) => set({ fallback_value })} />}
            </div>
        </div>
    );

    return (
        <BaseNode nodeData={nodeData} hidden={{ table: true, preview: true }} data={data}>
            {controls}
        </BaseNode>
    );
}

export default note_table;
