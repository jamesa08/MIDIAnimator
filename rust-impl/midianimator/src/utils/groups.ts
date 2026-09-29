// node groups in the frontend: reading and writing the graph at a group path, making and ungrouping groups, and for each zones.
// keep in sync with src-tauri/src/graph/{model,run,builtin}.rs
//
// EDGE DIRECTION: like the backend, stored edges are reversed from how data flows. `source`/`sourceHandle` is the
// node that CONSUMES the value (and its input), `target`/`targetHandle` is the node that PRODUCES it (and its output)

import { invoke } from "@tauri-apps/api/core";

export type Handle = { id: string; name: string; data_type: string; description?: string; hidden?: boolean };
export type Graph = { nodes: any[]; edges: any[]; viewport?: any; [key: string]: any };
export type GroupDef = Graph & { name: string; description?: string; interface: { inputs: Handle[]; outputs: Handle[] } };
export type Project = Graph & { groups?: Record<string, GroupDef> };

export const GROUP = "group";
export const GROUP_INPUT = "group_input";
export const GROUP_OUTPUT = "group_output";
export const FOR_EACH_INPUT = "for_each_input";
export const FOR_EACH_OUTPUT = "for_each_output";

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

// the project with the graph of `groupId` (null for the root) replaced. a built-in becomes the project's own copy
export function withGraph(project: Project, groups: Record<string, GroupDef>, groupId: string | null, graph: Graph): Project {
    const { nodes, edges, viewport } = graph;
    if (groupId === null) return { ...project, nodes, edges, viewport };
    return withGroup(project, groupId, { ...groups[groupId], nodes, edges, viewport });
}

// the project with a group definition added or replaced
export function withGroup(project: Project, groupId: string, def: GroupDef): Project {
    return { ...project, groups: { ...(project.groups ?? {}), [groupId]: def } };
}

// the project without its own copy of a group, a built-in goes back to the one that ships with the app
export function withoutGroup(project: Project, groupId: string): Project {
    const { [groupId]: _removed, ...groups } = project.groups ?? {};
    return { ...project, groups };
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

// MARK: - Ids

// short node ids: `{prefix}-{N}` where N is one more than the highest N already used for that prefix.
// older projects use `{type}-{uuid}` ids, those are ignored here and keep working.
// keep in sync with Graph::next_node_id in src-tauri/src/graph/model.rs
export function nextNodeId(nodes: { id: string }[], prefix: string): string {
    // find the highest N already used for this prefix
    const start = `${prefix}-`;
    let max = 0;
    for (const node of nodes) {
        if (!node.id.startsWith(start)) continue;
        // only count ids where the rest is a plain number (skips uuid ids)
        const rest = node.id.slice(start.length);
        if (/^\d+$/.test(rest)) max = Math.max(max, parseInt(rest, 10));
    }
    return `${start}${max + 1}`;
}

// the prefix new node ids get: group nodes are named after their group so ids stay readable (`evaluate_instrument-2`)
export function idPrefix(nodeType: string, data: any): string {
    return nodeType === GROUP && data?.group_id ? data.group_id : nodeType;
}

// a group id that isn't taken yet, `node_group`, `node_group_2`, ...
export function nextGroupId(groups: Record<string, GroupDef>): string {
    if (!groups.node_group) return "node_group";
    let n = 2;
    while (groups[`node_group_${n}`]) n++;
    return `node_group_${n}`;
}

// a group name that isn't taken yet, Blender style: `NodeGroup`, `NodeGroup.001`, ...
function nextGroupName(groups: Record<string, GroupDef>): string {
    const names = new Set(Object.values(groups).map((g) => g.name));
    if (!names.has("NodeGroup")) return "NodeGroup";
    let n = 1;
    while (names.has(`NodeGroup.${String(n).padStart(3, "0")}`)) n++;
    return `NodeGroup.${String(n).padStart(3, "0")}`;
}

// a socket id from a name that isn't in `taken` yet, `Object Map` -> `object_map`, `object_map_2`, ...
export function socketId(name: string, taken: Handle[]): string {
    const base =
        name
            .toLowerCase()
            .replace(/[^a-z0-9]+/g, "_")
            .replace(/^_|_$/g, "") || "socket";
    const ids = new Set(taken.map((h) => h.id));
    if (!ids.has(base) && base !== NEW_SOCKET) return base;
    let n = 2;
    while (ids.has(`${base}_${n}`)) n++;
    return `${base}_${n}`;
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

// MARK: - Make Group / Ungroup

// what making a group from a selection gives: the parent graph with the group node in place of the selection, and the new group
export type MadeGroup = { graph: Graph; groupId: string; def: GroupDef; nodeId: string };

// moves the selected nodes into a new group (Ctrl+G). connections crossing the selection become the group's sockets,
// each outside value that's used inside gets one input, each inside value used outside gets one output.
// null when the selection can't be grouped: nothing selected, the group input or output, or half a zone
export function makeGroup(graph: Graph, selected: Set<string>, lookup: SpecLookup, scope: GroupDef | null, groups: Record<string, GroupDef>): MadeGroup | null {
    const nodes = graph.nodes.filter((n) => selected.has(n.id));
    if (nodes.length === 0) return null;
    if (nodes.some((n) => n.type === GROUP_INPUT || n.type === GROUP_OUTPUT)) return null;
    // a zone has to stay in one piece
    for (const n of nodes) {
        if ((n.type === FOR_EACH_INPUT || n.type === FOR_EACH_OUTPUT) && n.data?.zone && !selected.has(n.data.zone)) return null;
    }

    const groupId = nextGroupId(groups);
    const inputs: Handle[] = [];
    const outputs: Handle[] = [];
    const inner: any[] = [];
    const outer: any[] = [];
    // one socket per outside output (inputs) or inside output (outputs), shared by every connection from it
    const inputFor = new Map<string, string>();
    const outputFor = new Map<string, string>();
    const groupNodeId = nextNodeId(graph.nodes, groupId);

    for (const edge of graph.edges) {
        const consumerIn = selected.has(edge.source);
        const producerIn = selected.has(edge.target);
        if (consumerIn && producerIn) {
            inner.push(edge);
        } else if (consumerIn) {
            // outside value used inside: group input socket
            const key = `${edge.target}\u0000${edge.targetHandle}`;
            let id = inputFor.get(key);
            if (!id) {
                const consumer = graph.nodes.find((n) => n.id === edge.source);
                const handle = inputHandle(lookup, consumer, edge.sourceHandle, scope);
                id = socketId(handle.name, inputs);
                inputs.push({ id, name: handle.name, data_type: handle.data_type, description: handle.description ?? "" });
                inputFor.set(key, id);
                outer.push({ ...edge, id: `xy-edge__${groupNodeId}${id}-${edge.target}${edge.targetHandle}`, source: groupNodeId, sourceHandle: id });
            }
            inner.push({ ...edge, id: `xy-edge__${edge.source}${edge.sourceHandle}-group_input-1${id}`, target: "group_input-1", targetHandle: id });
        } else if (producerIn) {
            // inside value used outside: group output socket
            const key = `${edge.target}\u0000${edge.targetHandle}`;
            let id = outputFor.get(key);
            if (!id) {
                const producer = graph.nodes.find((n) => n.id === edge.target);
                const handle = outputHandle(lookup, producer, edge.targetHandle, scope);
                id = socketId(handle.name, outputs);
                outputs.push({ id, name: handle.name, data_type: handle.data_type, description: handle.description ?? "" });
                outputFor.set(key, id);
                inner.push({ ...edge, id: `xy-edge__group_output-1${id}-${edge.target}${edge.targetHandle}`, source: "group_output-1", sourceHandle: id });
            }
            outer.push({ ...edge, id: `xy-edge__${edge.source}${edge.sourceHandle}-${groupNodeId}${id}`, target: groupNodeId, targetHandle: id });
        } else {
            outer.push(edge);
        }
    }

    // lay the group input and output out left and right of the grouped nodes, the group node goes where they were
    const xs = nodes.map((n) => n.position.x);
    const ys = nodes.map((n) => n.position.y);
    const right = Math.max(...nodes.map((n) => n.position.x + (n.measured?.width ?? 200)));
    const [minX, minY, maxY] = [Math.min(...xs), Math.min(...ys), Math.max(...ys)];
    const midY = (minY + maxY) / 2;
    const clean = ({ selected, dragging, measured, resizing, ...n }: any) => n;

    const def: GroupDef = {
        name: nextGroupName(groups),
        description: "",
        interface: { inputs, outputs },
        nodes: [{ id: "group_input-1", type: GROUP_INPUT, position: { x: minX - 300, y: midY }, data: { inputs: {} } }, ...nodes.map(clean), { id: "group_output-1", type: GROUP_OUTPUT, position: { x: right + 100, y: midY }, data: { inputs: {} } }],
        edges: inner,
    };
    const groupNode = { id: groupNodeId, type: GROUP, position: { x: minX, y: midY }, data: { group_id: groupId, inputs: {} }, selected: true };
    const parent = { ...graph, nodes: [...graph.nodes.filter((n) => !selected.has(n.id)).map((n) => ({ ...n, selected: false })), groupNode], edges: outer };
    return { graph: parent, groupId, def, nodeId: groupNodeId };
}

// replaces a group node with the nodes inside its group (Alt+G). connections through the group input and output are
// joined up directly. inner nodes whose ids are taken get new ones
export function ungroup(graph: Graph, nodeId: string, def: GroupDef): Graph {
    const groupNode = graph.nodes.find((n) => n.id === nodeId);
    if (!groupNode) return graph;
    const innerNodes = def.nodes.filter((n) => n.type !== GROUP_INPUT && n.type !== GROUP_OUTPUT);
    const boundary = new Set(def.nodes.filter((n) => n.type === GROUP_INPUT || n.type === GROUP_OUTPUT).map((n) => n.id));

    // new ids for inner nodes that clash with the parent's
    const taken: { id: string }[] = graph.nodes.filter((n) => n.id !== nodeId);
    const ids = new Map<string, string>();
    for (const node of innerNodes) {
        const id = taken.some((n) => n.id === node.id) ? nextNodeId(taken, idPrefix(node.type, node.data)) : node.id;
        ids.set(node.id, id);
        taken.push({ id });
    }

    // place the inner nodes around where the group node was, keeping their layout
    const xs = innerNodes.map((n) => n.position.x);
    const ys = innerNodes.map((n) => n.position.y);
    const [cx, cy] = [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2];
    const placed = innerNodes.map((n) => {
        const data = n.data?.zone ? { ...n.data, zone: ids.get(n.data.zone) ?? n.data.zone } : n.data;
        return { ...n, id: ids.get(n.id)!, data, position: { x: groupNode.position.x + n.position.x - cx, y: groupNode.position.y + n.position.y - cy }, selected: true };
    });

    // where each group socket's value comes from outside, and where each group output's value comes from inside
    const outsideInto = new Map(graph.edges.filter((e) => e.source === nodeId).map((e) => [e.sourceHandle, e]));
    const insideOut = new Map(def.edges.filter((e) => boundary.has(e.source)).map((e) => [e.sourceHandle, e]));

    const edges: any[] = [];
    const edge = (consumer: string, input: string, producer: string, output: string, base: any) => ({ ...base, id: `xy-edge__${consumer}${input}-${producer}${output}`, source: consumer, sourceHandle: input, target: producer, targetHandle: output });
    // the producer of a value inside the group, following a group input back out to the parent
    const producerOf = (target: string, targetHandle: string): [string, string] | null => {
        if (!boundary.has(target)) return [ids.get(target)!, targetHandle];
        const from = outsideInto.get(targetHandle);
        return from ? [from.target, from.targetHandle] : null;
    };

    for (const e of graph.edges) {
        if (e.source === nodeId) continue;
        if (e.target === nodeId) {
            // parent node reads a group output: connect it to whatever feeds that output inside
            const inner = insideOut.get(e.targetHandle);
            const producer = inner && producerOf(inner.target, inner.targetHandle);
            if (producer) edges.push(edge(e.source, e.sourceHandle, producer[0], producer[1], e));
        } else {
            edges.push(e);
        }
    }
    for (const e of def.edges) {
        if (boundary.has(e.source)) continue;
        const producer = producerOf(e.target, e.targetHandle);
        if (producer) edges.push(edge(ids.get(e.source)!, e.sourceHandle, producer[0], producer[1], e));
    }

    const nodes = [...graph.nodes.filter((n) => n.id !== nodeId).map((n) => ({ ...n, selected: false })), ...placed];
    return { ...graph, nodes, edges };
}
