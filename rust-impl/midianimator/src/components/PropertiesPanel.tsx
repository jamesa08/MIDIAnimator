import { ReactNode, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useStateContext } from "../contexts/StateContext";
import { Op, TabContext, useGraphOps } from "../utils/graphOps";
import { Handle, PATH_SEP, outputHandle, resolvePath, scopedValues, specLookup } from "../utils/groups";
import { useNodeSpecs } from "../utils/nodeEntries";
import { ERROR_KEY } from "./nodegraph/ErrorBadge";
import { usePanelGroups } from "./PanelBody";
import { overlapModes } from "../nodes/animation_generator";
import { noteNumbersText, noteToName } from "../nodes/assign_notes_to_objects";
import SceneTree from "./SceneTree";

// MARK: - Parameters

// what a parameter's editor can offer: the node and the values that went into it on the last run
type ParamContext = { node: any; executed: any };
type Option = { value: string; label: string };

// an input with an editor of its own instead of the one its type gets, like the widget on the node. `shown` hides it
// like the node does
type Param = { id: string; shown?: (node: any) => boolean } & ({ kind: "text" } | { kind: "number" } | { kind: "number_list" } | { kind: "select"; options: (ctx: ParamContext) => Option[] } | { kind: "midi_file" });

// the names of a list of named things, a mistyped connection can hand over anything
const names = (items: any): Option[] => (Array.isArray(items) ? items.map((item) => item?.name).filter((name) => typeof name === "string") : []).map((name) => ({ value: name, label: name }));

// each node type's inputs with an editor of their own (src/nodes), the rest get one by their type
const PARAMS: Record<string, Param[]> = {
    get_midi_file: [{ id: "file_path", kind: "midi_file" }],
    get_midi_track_data: [{ id: "track_name", kind: "select", options: ({ executed }) => names(executed?.tracks) }],
    keyframes_from_object: [
        { id: "object_group_name", kind: "select", options: ({ executed }) => names(executed?.object_groups) },
        { id: "object_name", kind: "select", options: ({ node, executed }) => names((Array.isArray(executed?.object_groups) ? executed.object_groups : []).find((g: any) => g?.name === node.data?.inputs?.object_group_name)?.objects) },
    ],
    assign_notes_to_objects: [
        {
            id: "mode",
            kind: "select",
            options: () => [
                { value: "rules", label: "Rules" },
                { value: "map", label: "Map" },
            ],
        },
        // drawn in the note map (Tab on the node)
        { id: "note_map", kind: "text", shown: () => false },
        { id: "map_layout", kind: "text", shown: () => false },
    ],
    animation_generator: [
        { id: "animation_overlap", kind: "select", options: () => overlapModes.map((mode) => ({ value: mode.id, label: mode.name })) },
        { id: "overlap_blend", kind: "number", shown: (node) => node.data?.inputs?.animation_overlap === "crossfade" },
    ],
};

// input types that can be typed in, the others only come from a connection
const TYPED: Record<string, Param["kind"]> = { String: "text", f64: "number", "Array<u8>": "number_list" };

// output types plain enough to show as text, alone or in a list
const PLAIN = ["String", "f64", "u8"];
const plain = (dataType: string) => PLAIN.includes(dataType.startsWith("Array<") ? dataType.slice(6, -1) : dataType);
// a list shows comma separated, nothing until the node has run
const plainText = (value: any): string => (Array.isArray(value) ? value.join(", ") : value === undefined || value === null ? "" : String(value));

// MARK: - Fields

// the value every node has, undefined when they differ (shown empty)
function shared<T>(nodes: any[], get: (node: any) => T): T | undefined {
    const first = get(nodes[0]);
    return nodes.every((node) => get(node) === first) ? first : undefined;
}

// a text box that sets its value when it's left or on enter, escape puts it back
function TextField({ value, onCommit, readOnly }: { value: string; onCommit: (value: string) => void; readOnly?: boolean }) {
    const [text, setText] = useState(value);
    useEffect(() => setText(value), [value]);

    const commit = () => {
        if (text !== value) onCommit(text);
    };

    return (
        <input
            value={text}
            readOnly={readOnly}
            onChange={(e) => setText(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
                if (e.key === "Escape") {
                    setText(value);
                    requestAnimationFrame(() => (e.target as HTMLInputElement).blur());
                }
            }}
        />
    );
}

// a number box, empty sets null when `empty` allows it, anything that isn't a number is put back
function NumberField({ value, onCommit, empty, readOnly }: { value: number | undefined; onCommit: (value: number | null) => void; empty?: boolean; readOnly?: boolean }) {
    const shown = value === undefined ? "" : String(value);
    return (
        <TextField
            value={shown}
            readOnly={readOnly}
            onCommit={(text) => {
                if (text.trim() === "") {
                    if (empty) onCommit(null);
                } else if (!isNaN(Number(text))) {
                    onCommit(Number(text));
                }
            }}
        />
    );
}

function SelectField({ value, options, onChange, readOnly }: { value: string | undefined; options: Option[]; onChange: (value: string) => void; readOnly?: boolean }) {
    // a value that isn't one of the options (gone from the scene, or nothing ran yet) still shows
    const all = value && !options.some((o) => o.value === value) ? [{ value, label: value }, ...options] : options;
    return (
        <select value={value ?? ""} disabled={readOnly} onChange={(e) => onChange(e.target.value)}>
            {!value && <option value="" />}
            {all.map((option) => (
                <option key={option.value} value={option.value}>
                    {option.label}
                </option>
            ))}
        </select>
    );
}

function MidiFileField({ value, onCommit, readOnly }: { value: string | undefined; onCommit: (value: string) => void; readOnly?: boolean }) {
    const pick = async () => {
        const picked = await open({ multiple: false, filters: [{ name: "", extensions: ["mid", "midi"] }] });
        if (picked != null) onCommit(picked.toString());
    };
    return (
        <button className="truncate" disabled={readOnly} onClick={pick}>
            {value ? value.split("/").pop() : "Pick MIDI File"}
        </button>
    );
}

// MARK: - Layout

// a section's bar, clicking it collapses or expands the section (also the nodes panel's categories)
export function SectionHeader({ title, collapsed, onToggle }: { title: string; collapsed: boolean; onToggle: () => void }) {
    return (
        <div className="properties-section" onClick={onToggle}>
            <svg height="6" viewBox="0 0 10 6" style={{ transform: collapsed ? "rotate(-90deg)" : undefined }}>
                <path d="M1 0.5l4 4 4-4" fill="none" stroke="currentColor" />
            </svg>
            <span>{title}</span>
        </div>
    );
}

// a collapsible group of rows
function Section({ title, collapsed, onToggle, children }: { title: string; collapsed: boolean; onToggle: () => void; children: ReactNode }) {
    return (
        <>
            <SectionHeader title={title} collapsed={collapsed} onToggle={onToggle} />
            {!collapsed && <div className="properties-rows">{children}</div>}
        </>
    );
}

// a name and its value, values that can't be changed are dimmed
function Row({ name, readOnly, children }: { name: string; readOnly?: boolean; children?: ReactNode }) {
    return (
        <div className={`properties-row${readOnly ? " read-only" : ""}`}>
            <span />
            <div className="properties-name">{name}</div>
            <div>{children}</div>
        </div>
    );
}

// a value that's only shown
function Value({ children }: { children: ReactNode }) {
    return <span className="px-1.5 truncate">{children}</span>;
}

// a value that's only shown, scrolls sideways when it's too long
function ScrollValue({ children }: { children: ReactNode }) {
    return <span className="properties-scroll px-1.5">{children}</span>;
}

// a box that's ticked or not, only shown
function Check({ checked }: { checked: boolean }) {
    return (
        <span className="properties-check">
            {checked && (
                <svg width="9" height="7" viewBox="0 0 9 7">
                    <path d="M0.5 3.5l2.5 2.5 5.5-5.5" fill="none" stroke="currentColor" />
                </svg>
            )}
        </span>
    );
}

// MARK: - Panel

// what's selected in the graph on screen. one node or several: the node, its parameters (the same values as its widgets,
// only when they're all the same type) and where it is. nothing selected: the project
function Properties() {
    const { backEndState: state } = useStateContext();
    const groups = usePanelGroups();
    const specs = useNodeSpecs();
    const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
    // the scene tree's open rows, the scene itself starts open
    const [expanded, setExpanded] = useState<Set<string>>(new Set(["scene"]));

    const project = state.rf_instance;
    const openGroup: string = state.open_group ?? "";
    const levels = useMemo(() => (project?.nodes ? resolvePath(project, groups, openGroup ? openGroup.split(PATH_SEP) : []) : []), [project, groups, openGroup]);
    const level = levels[levels.length - 1];
    const path = levels
        .slice(1)
        .map((l) => l.nodeId)
        .join(PATH_SEP);
    // a built-in group that hasn't been made local can't be edited
    const editable = !level || level.groupId === null || !!project.groups?.[level.groupId];
    const { apply } = useGraphOps(level?.groupId ?? null);

    if (!level) return null;

    const lookup = specLookup(Object.fromEntries(specs.map((spec: any) => [spec.id, spec])), groups);
    const selected: any[] = (level.graph.nodes ?? []).filter((node: any) => node.selected);
    const executedInputs = scopedValues(state.executed_inputs, path);
    const executedResults = scopedValues(state.executed_results, path);

    const toggle = (title: string) =>
        setCollapsed((prev) => {
            const next = new Set(prev);
            next.has(title) ? next.delete(title) : next.add(title);
            return next;
        });
    const toggleRow = (key: string) =>
        setExpanded((prev) => {
            const next = new Set(prev);
            next.has(key) ? next.delete(key) : next.add(key);
            return next;
        });
    const section = (title: string, children: ReactNode) => (
        <Section title={title} collapsed={collapsed.has(title)} onToggle={() => toggle(title)}>
            {children}
        </Section>
    );

    // one undo step for every selected node
    const edit = (op: (node: any) => Op | null) => {
        if (!editable) return;
        const ops = selected.map(op).filter((o): o is Op => o !== null);
        if (ops.length) apply(ops).catch((e) => console.error(`Error editing properties: ${e}`));
    };

    const typeName = (node: any): string => (lookup(node, level.def) as any)?.name ?? node.type;
    const body = selected.length === 0 ? projectSections() : nodeSections();

    function projectSections() {
        const tab = state.tabs?.find((t: any) => t.id === state.active_tab);
        const live = !!tab?.linked && !!state.connected;
        const errors = Object.entries(state.executed_results ?? {}).filter(([id, result]: [string, any]) => !id.includes(PATH_SEP) && result?.[ERROR_KEY]).length;
        return (
            <>
                {section(
                    "Project",
                    <>
                        <Row name="File" readOnly>
                            <Value>{tab?.path?.split("/").pop() ?? ""}</Value>
                        </Row>
                        <Row name="Nodes" readOnly>
                            <Value>{project.nodes?.length ?? 0}</Value>
                        </Row>
                        <Row name="Connections" readOnly>
                            <Value>{project.edges?.length ?? 0}</Value>
                        </Row>
                    </>
                )}
                {section(
                    "Blender",
                    <>
                        <Row name="File" readOnly>
                            <Value>{state.connected ? state.connected_file_name : ""}</Value>
                        </Row>
                        <Row name="Live" readOnly>
                            <Check checked={live} />
                        </Row>
                    </>
                )}
                {section(
                    "Last Run",
                    <Row name="Errors" readOnly>
                        <Value>{errors}</Value>
                    </Row>
                )}
            </>
        );
    }

    function nodeSections() {
        const single = selected.length === 1 ? selected[0] : null;
        const type = shared(selected, (node) => node.type);
        const groupId = shared(selected, (node) => node.data?.group_id);
        // parameters only when they're all the same kind of node: every input but the ones that grow with connections
        const spec = type !== undefined && groupId === selected[0].data?.group_id ? lookup(selected[0], level.def) : undefined;
        const custom = (id: string) => (PARAMS[type!] ?? []).find((param) => param.id === id);
        const inputs = (spec?.handles.inputs ?? []).filter((handle) => !handle.data_type.startsWith("Dyn<") && selected.every((node) => custom(handle.id)?.shown?.(node) ?? true));
        const label = shared(selected, (node) => node.data?.label ?? "");
        // the scene Scene Link gives (graph/execute.rs)
        const scene = state.scene_data?.["Scene"];
        const name = shared(selected, typeName);

        // what feeds an input: `Node › Output`, or null when nothing is connected to it.
        // stored edges are reversed, `source`/`sourceHandle` is the node taking the value and its input
        const source = (node: any, input: string): string | null => {
            const edge = (level.graph.edges ?? []).find((e: any) => e.source === node.id && e.sourceHandle === input);
            if (!edge) return null;
            const from = level.graph.nodes.find((n: any) => n.id === edge.target);
            if (!from) return null;
            return `${from.data?.label || typeName(from)} › ${outputHandle(lookup, from, edge.targetHandle, level.def).name}`;
        };

        const input = (handle: Handle) => {
            // a connected input takes the connection's value, the one set on the node is ignored
            const sources = selected.map((node) => source(node, handle.id));
            if (sources.some((s) => s !== null)) {
                return (
                    <Row key={handle.id} name={handle.name} readOnly>
                        <Value>{sources.every((s) => s === sources[0]) ? sources[0] : ""}</Value>
                    </Row>
                );
            }

            const param = custom(handle.id) ?? (TYPED[handle.data_type] ? ({ id: handle.id, kind: TYPED[handle.data_type] } as Param) : null);
            if (!param) {
                return <Row key={handle.id} name={handle.name} readOnly />;
            }

            // an input that isn't set shows the value it runs with, its default
            const values = selected.map((node) => node.data?.inputs?.[param.id] ?? handle.default);
            // lists are compared by what's in them
            const mixed = values.some((v) => JSON.stringify(v) !== JSON.stringify(values[0]));
            const value = mixed ? undefined : values[0];
            // an emptied text input is unset like a number, so it's left to the node's default
            const set = (v: any) => edit((node) => ({ op: "set_inputs", node: node.id, inputs: { [param.id]: v === "" ? null : v } }));
            let field: ReactNode;
            switch (param.kind) {
                case "text":
                    field = <TextField value={value ?? ""} readOnly={!editable} onCommit={set} />;
                    break;
                case "number":
                    field = <NumberField value={value} empty readOnly={!editable} onCommit={set} />;
                    break;
                case "number_list":
                    // kept as typed, the node reads the numbers out of it when it runs
                    field = <TextField value={noteNumbersText(value)} readOnly={!editable} onCommit={set} />;
                    break;
                case "select":
                    field = <SelectField value={value} options={param.options({ node: selected[0], executed: executedInputs[selected[0].id] })} readOnly={!editable} onChange={set} />;
                    break;
                case "midi_file":
                    field = <MidiFileField value={value} readOnly={!editable} onCommit={set} />;
                    break;
            }
            return (
                <Row key={handle.id} name={handle.name} readOnly={!editable}>
                    {field}
                </Row>
            );
        };

        // a single node's outputs that read as text, with what they gave on the last run
        const outputs = single && spec ? spec.handles.outputs.filter((handle) => !handle.hidden && plain(handle.data_type)) : [];

        // the notes Assign Notes to Objects gave each object on the last run, from every animation, like the old add-on's
        // dialog (Object: Cube_60 => Note: 60/C3). a mistyped connection can hand over anything
        const objectMap = single?.type === "assign_notes_to_objects" ? executedResults[single.id]?.object_map?.objects : undefined;
        const objectNotes =
            objectMap && typeof objectMap === "object"
                ? Object.entries(objectMap)
                      .map(([object, animations]: [string, any]) => [object, [...new Set(Object.values(animations ?? {}).flat())].filter((n): n is number => typeof n === "number")] as const)
                      .sort(([a], [b]) => a.localeCompare(b, undefined, { numeric: true }))
                : null;

        // where a node is, rounded like the canvas shows it
        const x = shared(selected, (node) => Math.round(node.position.x));
        const y = shared(selected, (node) => Math.round(node.position.y));
        const move = (axis: "x" | "y", v: number | null) => {
            if (v === null || !editable) return;
            apply([{ op: "move", positions: Object.fromEntries(selected.map((node) => [node.id, { ...node.position, [axis]: v }])) }]).catch((e) => console.error(`Error moving nodes: ${e}`));
        };

        return (
            <>
                {section(
                    "Node",
                    <>
                        <Row name="Type" readOnly>
                            <Value>{name ?? ""}</Value>
                        </Row>
                        <Row name="Label" readOnly={!editable}>
                            <TextField value={label ?? ""} readOnly={!editable} onCommit={(v) => edit((node) => ({ op: "set_label", node: node.id, label: v.trim() }))} />
                        </Row>
                        {single && (
                            <Row name="ID" readOnly>
                                <Value>{single.id}</Value>
                            </Row>
                        )}
                    </>
                )}
                {inputs.length > 0 && section("Parameters", inputs.map(input))}
                {outputs.length > 0 &&
                    section(
                        "Outputs",
                        outputs.map((handle) => (
                            <Row key={handle.id} name={handle.name} readOnly>
                                <ScrollValue>{plainText(executedResults[single.id]?.[handle.id])}</ScrollValue>
                            </Row>
                        ))
                    )}
                {objectNotes &&
                    objectNotes.length > 0 &&
                    section(
                        "ObjectMap",
                        objectNotes.map(([object, notes]) => (
                            <Row key={object} name={object} readOnly>
                                <ScrollValue>{notes.map((n) => `${n}/${noteToName(n)}`).join(", ")}</ScrollValue>
                            </Row>
                        ))
                    )}
                {single?.type === "scene_link" && scene && section("Scene", <SceneTree scene={scene} expanded={expanded} onToggle={toggleRow} />)}
                {section(
                    "Layout",
                    <>
                        <Row name="X" readOnly={!editable}>
                            <NumberField value={x} readOnly={!editable} onCommit={(v) => move("x", v)} />
                        </Row>
                        <Row name="Y" readOnly={!editable}>
                            <NumberField value={y} readOnly={!editable} onCommit={(v) => move("y", v)} />
                        </Row>
                    </>
                )}
            </>
        );
    }

    // fields start over when the selection changes
    return (
        <div key={selected.map((node) => node.id).join(",")} className="properties">
            {body}
        </div>
    );
}

// the properties panel, its edits go to the tab on screen
function PropertiesPanel() {
    const { backEndState: state } = useStateContext();
    return (
        <TabContext.Provider value={state.active_tab ?? ""}>
            <Properties />
        </TabContext.Provider>
    );
}

export default PropertiesPanel;
