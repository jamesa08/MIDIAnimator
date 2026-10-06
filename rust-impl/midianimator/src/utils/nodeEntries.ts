// the nodes that can be added (add menu, nodes panel): every node type, plus one entry per node group

import { useContext, useEffect, useState } from "react";
import nodeTypes from "../nodes/NodeTypes";
import { CATEGORY_ORDER } from "../styles";
import { StateContext } from "../contexts/StateContext";
import { loadedNodeSpecs, loadNodeSpecs } from "./node";
import { FOR_EACH_INPUT, FOR_EACH_OUTPUT, GROUP, GROUP_INPUT, GROUP_OUTPUT, GroupDef, Handle, NEW_SOCKET } from "./groups";
import { compatible } from "./sockets";

// `key` is what the menus and drops pass around: a node type, or a group id for a group node
export type NodeEntry = { key: string; label: string; nodeType: string; category: string; data: any };

// node types that aren't added on their own: group nodes are added by their group, for each adds both of its ends,
// and the group input and output only make sense inside a group
const NOT_LISTED = new Set(["group", "for_each_output", GROUP_INPUT, GROUP_OUTPUT]);

const LABELS: Record<string, string> = { [FOR_EACH_INPUT]: "For Each" };

function label(nodeType: string): string {
    return LABELS[nodeType] ?? nodeType.replace(/_/g, " ").replace(/\b\w/g, (l) => l.toUpperCase());
}

// the node specs, from the state when this window has it.
// read synchronously so a node is drawn filled in on its first frame
export function useNodeSpecs(): any[] {
    const fromState = useContext(StateContext)?.backEndState?.default_nodes?.nodes;
    const [loaded, setLoaded] = useState<any[] | null>(loadedNodeSpecs);
    useEffect(() => {
        if (!fromState && !loaded) loadNodeSpecs().then(setLoaded);
    }, [fromState, loaded]);
    return fromState ?? loaded ?? [];
}

// one node type's spec, null until the specs are read
export function useNodeSpec(nodeType: string): any | null {
    return useNodeSpecs().find((spec) => spec.id === nodeType) ?? null;
}

// `specs` gives each entry its name (the node's header) and category, entries are sorted by category.
// `inGroup` adds the group input and output, `exclude` leaves out groups that would end up containing themselves
export function nodeEntries(groups: Record<string, GroupDef>, specs: any[], inGroup = false, exclude: Set<string> = new Set()): NodeEntry[] {
    const spec = (nodeType: string) => specs.find((spec) => spec.id === nodeType);
    const entries: NodeEntry[] = Object.keys(nodeTypes)
        .filter((nodeType) => !NOT_LISTED.has(nodeType))
        .map((nodeType) => ({ key: nodeType, label: spec(nodeType)?.name ?? label(nodeType), nodeType, category: spec(nodeType)?.category ?? "", data: {} }));
    if (inGroup) {
        entries.push({ key: GROUP_INPUT, label: "Group Input", nodeType: GROUP_INPUT, category: "group", data: {} }, { key: GROUP_OUTPUT, label: "Group Output", nodeType: GROUP_OUTPUT, category: "group", data: {} });
    }
    for (const [groupId, def] of Object.entries(groups)) {
        if (!exclude.has(groupId)) entries.push({ key: groupId, label: def.name, nodeType: "group", category: "group", data: { group_id: groupId } });
    }
    // unknown categories go last, sort is stable so entries keep their order within a category
    const rank = (entry: NodeEntry) => (CATEGORY_ORDER.includes(entry.category) ? CATEGORY_ORDER.indexOf(entry.category) : CATEGORY_ORDER.length);
    return entries.sort((a, b) => rank(a) - rank(b));
}

// what a preview of an entry is drawn with (nodes panel, drag ghost)
export function previewData(entry: NodeEntry): any {
    return Object.keys(entry.data).length > 0 ? { ...entry.data, preview: true } : "preview";
}

// a link dragged off a socket and dropped on nothing: the node and socket it came from, and that socket's type
export type LinkFrom = { nodeId: string; handleId: string; isOutput: boolean; dataType: string };

// the socket on an entry's new node that a link from `from` connects to, null when none fits. exact types come before
// ones that only fit through `Any`, then the first in order. `node` is which of the added nodes it's on (a for each adds
// its input, then its output)
export function linkSocket(entry: NodeEntry, from: LinkFrom, specs: any[], groups: Record<string, GroupDef>): { node: number; handle: string } | null {
    const side = from.isOutput ? "inputs" : "outputs";
    const specHandles = (nodeType: string): Handle[] => specs.find((spec) => spec.id === nodeType)?.handles?.[side] ?? [];

    let candidates: { node: number; handle: Handle }[];
    if (entry.nodeType === GROUP) {
        candidates = (groups[entry.key]?.interface[side] ?? []).map((handle) => ({ node: 0, handle }));
    } else if (entry.nodeType === GROUP_INPUT || entry.nodeType === GROUP_OUTPUT) {
        // only the empty socket, connecting to it adds a group socket typed after the other end. not from another empty socket
        const open = entry.nodeType === GROUP_INPUT ? "outputs" : "inputs";
        candidates = side === open && from.handleId !== NEW_SOCKET ? [{ node: 0, handle: { id: NEW_SOCKET, name: "", data_type: "Any" } }] : [];
    } else if (entry.nodeType === FOR_EACH_INPUT) {
        candidates = [...specHandles(FOR_EACH_INPUT).map((handle) => ({ node: 0, handle })), ...specHandles(FOR_EACH_OUTPUT).map((handle) => ({ node: 1, handle }))];
    } else {
        candidates = specHandles(entry.nodeType).map((handle) => ({ node: 0, handle }));
    }

    // parameters and dynamic outputs can't be connected, a dynamic input is connected through its first numbered one
    const sockets = candidates.filter(({ handle }) => !handle.hidden && !(side === "outputs" && handle.data_type.startsWith("Dyn<"))).map(({ node, handle }) => (handle.data_type.startsWith("Dyn<") ? { node, id: `${handle.id}_0`, type: handle.data_type.slice(4, -1) } : { node, id: handle.id, type: handle.data_type }));

    const rank = (type: string) => {
        const [outType, inType] = from.isOutput ? [from.dataType, type] : [type, from.dataType];
        if (!compatible(outType, inType)) return Infinity;
        if (outType === inType) return 0;
        return outType.includes("Any") || inType.includes("Any") ? 2 : 1;
    };
    let best: { node: number; handle: string } | null = null;
    let bestRank = Infinity;
    for (const socket of sockets) {
        const r = rank(socket.type);
        if (r < bestRank) {
            best = { node: socket.node, handle: socket.id };
            bestRank = r;
        }
    }
    return best;
}
