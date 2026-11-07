// socket categories: the color says what kind of value a handle carries

import { SOCKET_COLORS } from "../styles";

// element type (what's inside any Array or HashMap) to its category
const CATEGORIES: Record<string, keyof typeof SOCKET_COLORS> = {
    MIDITrack: "midi",
    MIDINote: "midi",
    MIDIEvent: "midi",
    ObjectGroup: "scene",
    AnimationGenerator: "generator",
    ObjectMap: "object_map",
    NoteTarget: "target",
    Keyframe: "keyframes",
    CurveKeys: "keyframes",
    BlendKeyframe: "keyframes",
    f64: "number",
    u8: "number",
    String: "string",
    Any: "any",
};

// the innermost element type of a data type, e.g. "HashMap<u8, Array<NoteTarget>>" is NoteTarget
function elementType(dataType: string): string {
    let type = dataType.trim();
    while (true) {
        if (type.startsWith("Dyn<") || type.startsWith("Array<")) {
            type = type.slice(type.indexOf("<") + 1, -1);
        } else if (type.startsWith("HashMap<")) {
            type = type.slice(type.indexOf(",") + 1, -1).trim();
        } else {
            return type;
        }
    }
}

export function socketCategory(dataType: string): keyof typeof SOCKET_COLORS {
    return CATEGORIES[elementType(dataType)] ?? "any";
}

// style for a handle carrying `dataType`, merged over the handle's position style
export function socketStyle(dataType: string | undefined): Record<string, any> {
    return dataType ? { background: SOCKET_COLORS[socketCategory(dataType)] } : {};
}

// MARK: - Multi Inputs

// a multi input (Blender's) takes any number of links. its socket is a pill with a slot per link, never less than two so
// it always looks like one, and each link ends at its own slot in the order they were made
const SOCKET_SIZE = 14;
export const MULTI_SLOT = 10;

// the size of a multi input's socket with `links` links
export function multiSocketStyle(links: number): Record<string, any> {
    return { height: `${SOCKET_SIZE + (Math.max(links, 2) - 1) * MULTI_SLOT}px`, borderRadius: `${SOCKET_SIZE / 2}px` };
}

// how far from the socket's middle the link at `index` of `links` ends
export const multiSlotOffset = (index: number, links: number) => (index - (links - 1) / 2) * MULTI_SLOT;

// whether an output of type `outType` can feed an input of type `inType`. keep in sync with compatible in src-tauri/src/graph/model.rs
export function compatible(outType: string, inType: string): boolean {
    const array = (type: string) => (type.startsWith("Array<") && type.endsWith(">") ? type.slice(6, -1).trim() : null);
    if (outType === inType || inType === "Any" || outType === "Any" || outType.startsWith("Dyn<")) return true;
    const outInner = array(outType);
    const inInner = array(inType);
    return outInner !== null && inInner !== null && compatible(outInner, inInner);
}

// whether connecting `fromOutput` on `fromNode` to `toInput` on `toNode` makes a cycle, socket by socket like the backend
// (Graph::would_cycle in model.rs). `reads` gives the inputs an output reads when it's a partial output (`reads` in
// default_nodes.json), null for one that reads them all: it only carries on from those, so a loop back into an input it
// doesn't read isn't a cycle. `links` go as data flows
export function wouldCycle(links: { from: string; output: string; to: string; input: string }[], reads: (node: string, output: string) => string[] | null, fromNode: string, fromOutput: string, toNode: string, toInput: string): boolean {
    if (fromNode === toNode) return true;
    const carries = (node: string, output: string, input: string) => reads(node, output)?.includes(input) ?? true;
    const seen = new Set<string>();
    const queue: [string, string][] = [[toNode, toInput]];
    while (queue.length > 0) {
        const [node, input] = queue.shift()!;
        if (node === fromNode && carries(node, fromOutput, input)) return true;
        const key = `${node}\n${input}`;
        if (seen.has(key)) continue;
        seen.add(key);
        for (const l of links) if (l.from === node && carries(node, l.output, input)) queue.push([l.to, l.input]);
    }
    return false;
}
