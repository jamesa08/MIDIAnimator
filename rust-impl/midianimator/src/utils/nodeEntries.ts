// the nodes that can be added (add menu, nodes panel): every node type, plus one entry per node group

import nodeTypes from "../nodes/NodeTypes";
import { FOR_EACH_INPUT, GROUP_INPUT, GROUP_OUTPUT, GroupDef } from "./groups";

// `key` is what the menus and drops pass around: a node type, or a group id for a group node
export type NodeEntry = { key: string; label: string; nodeType: string; data: any };

// node types that aren't added on their own: group nodes are added by their group, for each adds both of its ends,
// and the group input and output only make sense inside a group
const NOT_LISTED = new Set(["group", "for_each_output", GROUP_INPUT, GROUP_OUTPUT]);

const LABELS: Record<string, string> = { [FOR_EACH_INPUT]: "For Each" };

function label(nodeType: string): string {
    return LABELS[nodeType] ?? nodeType.replace(/_/g, " ").replace(/\b\w/g, (l) => l.toUpperCase());
}

// `inGroup` adds the group input and output, `exclude` leaves out groups that would end up containing themselves
export function nodeEntries(groups: Record<string, GroupDef>, inGroup = false, exclude: Set<string> = new Set()): NodeEntry[] {
    const entries: NodeEntry[] = Object.keys(nodeTypes)
        .filter((nodeType) => !NOT_LISTED.has(nodeType))
        .map((nodeType) => ({ key: nodeType, label: label(nodeType), nodeType, data: {} }));
    if (inGroup) {
        entries.push({ key: GROUP_INPUT, label: "Group Input", nodeType: GROUP_INPUT, data: {} }, { key: GROUP_OUTPUT, label: "Group Output", nodeType: GROUP_OUTPUT, data: {} });
    }
    for (const [groupId, def] of Object.entries(groups)) {
        if (!exclude.has(groupId)) entries.push({ key: groupId, label: def.name, nodeType: "group", data: { group_id: groupId } });
    }
    return entries;
}

// what a preview of an entry is drawn with (nodes panel, drag ghost)
export function previewData(entry: NodeEntry): any {
    return Object.keys(entry.data).length > 0 ? { ...entry.data, preview: true } : "preview";
}
