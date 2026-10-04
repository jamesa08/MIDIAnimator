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

// whether an output of type `outType` can feed an input of type `inType`. keep in sync with compatible in src-tauri/src/graph/model.rs
export function compatible(outType: string, inType: string): boolean {
    const array = (type: string) => (type.startsWith("Array<") && type.endsWith(">") ? type.slice(6, -1).trim() : null);
    if (outType === inType || inType === "Any" || outType === "Any" || outType.startsWith("Dyn<")) return true;
    const outInner = array(outType);
    const inInner = array(inType);
    return outInner !== null && inInner !== null && compatible(outInner, inInner);
}
