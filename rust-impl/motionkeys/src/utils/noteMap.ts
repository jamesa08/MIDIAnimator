// the note map of Assign Notes to Objects as data: what it shows, read from the node's inputs and its last run. no
// components here, the node's preview (NoteMapPreview.tsx) and the map (NoteMapView.tsx) both read it. keep in sync with
// assign_notes_to_objects and NoteMap in src-tauri/src/graph/executors/animation.rs

// each object's notes, the objects each animation plays on (by the generator's name), and notes added that nothing is
// connected to yet
export type NoteMap = { objects: Record<string, number[]>; notes: number[]; animations: Record<string, string[]> };
// what the map shows from the node's last run: its object group's objects in name order, the MIDI's notes, the
// connected animations' names in connect order, and each object's notes the node gave (what the rules gave, in rules
// mode)
export type MapData = { objects: string[]; midiNotes: number[]; animations: string[]; assigned: Record<string, number[]> };
// which wires the map and its preview show, kept on the node as `map_view` (unset is both)
export type MapView = "notes" | "animations" | "combined";

export const byNumber = (a: number, b: number) => a - b;
export const unique = (notes: number[]) => [...new Set(notes)].sort(byNumber);
const list = (value: any): any[] => (Array.isArray(value) ? value : []);
const notesIn = (value: any): number[] => list(value).filter((n) => Number.isInteger(n));

// the view stored on a node, anything else is both
export const mapView = (value: any): MapView => (value === "notes" || value === "animations" ? value : "combined");

// the inputs and outputs the node ran with, a mistyped connection can hand over anything
export function noteMapData(inputs: any, results: any, groupName: string | undefined): MapData {
    const group = list(inputs?.object_groups).find((g) => g?.name === groupName);
    const objects = list(group?.objects)
        .map((o) => o?.name)
        .filter((name) => typeof name === "string")
        .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
    const midiNotes = unique(
        list(inputs?.midi_notes)
            .map((n) => n?.note_number)
            .filter(Number.isInteger)
    );
    // the generators are a list (a multi input), one on its own is from before they were
    const generators = Array.isArray(inputs?.generator) ? inputs.generator : inputs?.generator ? [inputs.generator] : [];
    const animations = [...new Set<string>(generators.map((g: any) => g?.name).filter((name: any) => typeof name === "string"))];
    // the Note Map output has every object's notes, the object map only the ones that play an animation
    const assigned: Record<string, number[]> = {};
    if (results?.object_notes && typeof results.object_notes === "object") {
        for (const [name, notes] of Object.entries(results.object_notes)) assigned[name] = unique(notesIn(notes));
    } else {
        for (const [name, played] of Object.entries(results?.object_map?.objects ?? {})) {
            assigned[name] = unique(Object.values(played ?? {}).flatMap(notesIn));
        }
    }
    return { objects, midiNotes, animations, assigned };
}

// what the map shows: the rules' result in rules mode, the stored map (`noteMap`, the node's note_map) in map mode, and
// every note in it in order. the rules play every animation on every object, and so does a map from before animations
// were wired in it. the node's preview (NoteMapPreview.tsx) shows the same
export function shownMap(mode: string, info: MapData, noteMap: any): { map: NoteMap; notes: number[] } {
    const everywhere = () => Object.fromEntries(info.animations.map((name) => [name, [...info.objects]]));
    let map: NoteMap;
    if (mode !== "map") {
        map = readMap({ objects: info.assigned }, everywhere);
        // padded notes the MIDI doesn't play stay in the map once it's edited, even with nothing connected
        map.notes = Object.values(map.objects)
            .flat()
            .filter((n) => !info.midiNotes.includes(n));
    } else {
        map = readMap(noteMap, everywhere);
    }
    const notes = unique([...info.midiNotes, ...map.notes, ...info.objects.flatMap((o) => map.objects[o] ?? [])]);
    return { map, notes };
}

// the animation nodes the map shows: every connected animation in connect order, then the ones wired in the map whose
// generator isn't connected any more
export const shownAnimations = (map: NoteMap, info: MapData): string[] => [...info.animations, ...Object.keys(map.animations).filter((name) => !info.animations.includes(name))];

// true if `note` is wired to one of `objects`. with linked only (the node's `linked_only`) the map and its preview show
// only these, and the animations `isAnimationLinked` says are
export const isLinked = (map: NoteMap, objects: string[], note: number) => objects.some((o) => map.objects[o]?.includes(note));
export const isAnimationLinked = (map: NoteMap, objects: string[], name: string) => (map.animations[name] ?? []).some((o) => objects.includes(o));

// a stored map as given, anything that isn't one is empty. one without animations gets `animations()`'s
export function readMap(value: any, animations: () => Record<string, string[]> = () => ({})): NoteMap {
    const objects: Record<string, number[]> = {};
    for (const [name, notes] of Object.entries(value?.objects ?? {})) {
        if (Array.isArray(notes)) objects[name] = notesIn(notes);
    }
    let wired: Record<string, string[]>;
    if (value?.animations && typeof value.animations === "object" && !Array.isArray(value.animations)) {
        wired = {};
        for (const [name, on] of Object.entries(value.animations)) wired[name] = list(on).filter((o) => typeof o === "string");
    } else {
        wired = animations();
    }
    return { objects, notes: notesIn(value?.notes), animations: wired };
}
