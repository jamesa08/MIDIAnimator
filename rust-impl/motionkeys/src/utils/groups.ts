// node groups in the frontend: reading the graph at a group path, socket specs, and for each zones. editing groups
// (making, ungrouping, sockets) is done by the backend, src-tauri/src/graph/ops.rs. keep in sync with src-tauri/src/graph/{model,run,builtin}.rs
//
// EDGE DIRECTION: like the backend, stored edges are reversed from how data flows. `source`/`sourceHandle` is the
// node that CONSUMES the value (and its input), `target`/`targetHandle` is the node that PRODUCES it (and its output)

import { invoke } from "@tauri-apps/api/core";

export type Handle = { id: string; name: string; data_type: string; description?: string; hidden?: boolean; multi?: boolean; default?: any };
export type Graph = { nodes: any[]; edges: any[]; viewport?: any; [key: string]: any };
export type GroupDef = Graph & { name: string; description?: string; category?: string; interface: { inputs: Handle[]; outputs: Handle[] } };
export type Project = Graph & { groups?: Record<string, GroupDef> };

export const GROUP = "group";
export const GROUP_INPUT = "group_input";
export const GROUP_OUTPUT = "group_output";
export const FOR_EACH_INPUT = "for_each_input";
export const FOR_EACH_OUTPUT = "for_each_output";
// Tab on it opens its note map (NoteMapView.tsx) instead of a group
export const NOTE_MAP_NODE = "assign_notes_to_objects";

// separates the group node ids in a path, `evaluate_instrument-1/node_group-2`
export const PATH_SEP = "/";

// the socket at the end of a group input or output that adds a new socket to the group when something is connected to it
export const NEW_SOCKET = "__new__";

// MARK: - Built-in Groups

let builtinPromise: Promise<Record<string, GroupDef>> | null = null;

// the groups that ship with the app, they're the same for every project so they're fetched once
export function loadBuiltinGroups(): Promise<Record<string, GroupDef>> {
    if (!builtinPromise) {
        builtinPromise = invoke<Record<string, GroupDef>>("get_builtin_groups").catch((e) => {
            console.error("could not load the built-in node groups", e);
            builtinPromise = null;
            return {};
        });
    }
    return builtinPromise;
}

// the groups a project can use: the built-ins, replaced by the project's own copies
export function allGroups(project: Project | undefined, builtin: Record<string, GroupDef>): Record<string, GroupDef> {
    return { ...builtin, ...(project?.groups ?? {}) };
}

// MARK: - Paths

// one graph on the way down a path: the root (groupId null) or the inside of a group node
export type Level = { graph: Graph; groupId: string | null; def: GroupDef | null; nodeId: string | null };

// the graphs along a path, it stops early where a group node or group is missing (deleted, or the project changed)
export function resolvePath(project: Project, groups: Record<string, GroupDef>, path: string[]): Level[] {
    const levels: Level[] = [{ graph: project, groupId: null, def: null, nodeId: null }];
    for (const nodeId of path) {
        const node = levels[levels.length - 1].graph.nodes?.find((n: any) => n.id === nodeId);
        const groupId = node?.type === GROUP ? node.data?.group_id : undefined;
        const def = groupId ? groups[groupId] : undefined;
        // a group can't contain itself, stop instead of looping
        if (!def || levels.some((l) => l.groupId === groupId)) break;
        levels.push({ graph: def, groupId, def, nodeId });
    }
    return levels;
}

// paths inside the group node at `path`, without its path. paths deeper in keep the group nodes they're under
export function scopedPaths(paths: string[] | undefined, path: string): string[] {
    if (!paths || path === "") return paths ?? [];
    const prefix = path + PATH_SEP;
    return paths.filter((p) => p.startsWith(prefix)).map((p) => p.slice(prefix.length));
}

// executed values recorded inside the group node at `path`, keyed by the inner node ids
export function scopedValues(values: Record<string, any> | undefined, path: string): Record<string, any> {
    if (!values || path === "") return values ?? {};
    const prefix = path + PATH_SEP;
    const scoped: Record<string, any> = {};
    for (const [key, value] of Object.entries(values)) {
        if (!key.startsWith(prefix)) continue;
        const rest = key.slice(prefix.length);
        if (!rest.includes(PATH_SEP)) scoped[rest] = value;
    }
    return scoped;
}

// MARK: - Handles

export type SpecLookup = (node: any, scope: GroupDef | null) => { handles: { inputs: Handle[]; outputs: Handle[] } } | undefined;

// the specs of the nodes in one graph, group nodes and the group input and output get their sockets from the group.
// keep in sync with Specs::for_node in src-tauri/src/graph/model.rs
export function specLookup(specs: Record<string, any>, groups: Record<string, GroupDef>): SpecLookup {
    return (node, scope) => {
        const spec = specs[node.type];
        if (!spec) return undefined;
        if (node.type === GROUP) {
            const def = groups[node.data?.group_id];
            return def ? { ...spec, name: def.name, handles: { inputs: def.interface.inputs, outputs: def.interface.outputs } } : spec;
        }
        if (node.type === GROUP_INPUT) return { ...spec, handles: { inputs: [], outputs: scope?.interface.inputs ?? [] } };
        if (node.type === GROUP_OUTPUT) return { ...spec, handles: { inputs: scope?.interface.outputs ?? [], outputs: [] } };
        return spec;
    };
}

// the spec of one handle, `Dyn<T>` inputs (object_maps_0, ...) and outputs (location_z, ...) fall back to their base
function findHandle(handles: Handle[], id: string): Handle | undefined {
    return handles.find((h) => h.id === id) ?? handles.find((h) => h.data_type.startsWith("Dyn<") && id.startsWith(h.id + "_")) ?? handles.find((h) => h.data_type.startsWith("Dyn<"));
}

export function inputHandle(lookup: SpecLookup, node: any, id: string, scope: GroupDef | null): Handle {
    const found = findHandle(lookup(node, scope)?.handles.inputs ?? [], id);
    return found ? { ...found, data_type: dynInner(found.data_type) } : { id, name: id, data_type: "Any" };
}

export function outputHandle(lookup: SpecLookup, node: any, id: string, scope: GroupDef | null): Handle {
    const found = findHandle(lookup(node, scope)?.handles.outputs ?? [], id);
    return found ? { ...found, data_type: dynInner(found.data_type) } : { id, name: id, data_type: "Any" };
}

function dynInner(dataType: string): string {
    return dataType.startsWith("Dyn<") ? dataType.slice(4, -1) : dataType;
}

// MARK: - Zones

export type Zone = { input: string; output: string; body: Set<string> };

// the for each zones in a graph and the nodes inside each: nodes that depend on the zone input, except the output and
// anything after it. keep in sync with Runner::find_zones in src-tauri/src/graph/run.rs
export function findZones(graph: Graph): Zone[] {
    const ids = new Set((graph.nodes ?? []).map((n: any) => n.id));
    const descendants = (start: string) => {
        const seen = new Set<string>();
        const stack = [start];
        while (stack.length) {
            const id = stack.pop()!;
            // data flows from the edge's target (producer) to its source (consumer)
            for (const edge of graph.edges ?? []) {
                if (edge.target === id && !seen.has(edge.source)) {
                    seen.add(edge.source);
                    stack.push(edge.source);
                }
            }
        }
        return seen;
    };

    const zones: Zone[] = [];
    for (const node of graph.nodes ?? []) {
        if (node.type !== FOR_EACH_INPUT) continue;
        const output = node.data?.zone;
        if (!output || !ids.has(output)) continue;
        const afterOutput = descendants(output);
        const body = new Set([...descendants(node.id)].filter((id) => id !== output && !afterOutput.has(id)));
        zones.push({ input: node.id, output, body });
    }
    return zones;
}
