import { findGroup } from "./objectGroups";
// the note map of Assign Notes to Objects as data: what it shows, read from the node's inputs and its last run. no
// components here, the node's preview (NoteMapPreview.tsx) and the map (NoteMapView.tsx) both read it. keep in sync with
// assign_notes_to_objects and NoteMap in src-tauri/src/graph/executors/animation.rs

// each object's notes, the objects each animation plays on (by the generator's name, or a combine's key), notes added
// that nothing is connected to yet, and the combines drawn in the map by their keys
export type NoteMap = { objects: Record<string, number[]>; notes: number[]; animations: Record<string, string[]>; combines: Record<string, MapCombine> };
// a combine drawn in the map, like Combine Animations: the animations going into it in order (generators' names or
// other combines' keys) and how they combine, unset is add
export type MapCombine = { inputs: string[]; animation_overlap?: string };
// what the map shows from the node's last run: its object group's objects (their ids) in name order with their names,
// the MIDI's notes, the connected animations' names in connect order, and each object's notes the node gave (what the
// rules gave, in rules mode). objects are kept by id everywhere, the map too, and shown by name
export type MapData = { objects: string[]; objectNames: Record<string, string>; midiNotes: number[]; animations: string[]; assigned: Record<string, number[]> };
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
    const group = findGroup(list(inputs?.object_groups), groupName);
    const named = list(group?.objects)
        .filter((o) => typeof o?.id === "string")
        .map((o) => ({ id: o.id as string, name: typeof o.name === "string" ? (o.name as string) : (o.id as string) }))
        .sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
    const objects = named.map((o) => o.id);
    const objectNames = Object.fromEntries(named.map((o) => [o.id, o.name]));
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
    return { objects, objectNames, midiNotes, animations, assigned };
}

// what the map shows: the rules' result in rules mode, the stored map (`noteMap`, the node's note_map) in map mode, and
// every note in it in order. the rules play every animation on every object, and so does a map on a generator it has no
// wires for (connected after the map was drawn). the node's preview (NoteMapPreview.tsx) shows the same
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
export const shownAnimations = (map: NoteMap, info: MapData): string[] => {
    const combines = Object.keys(map.combines).filter((key) => !info.animations.includes(key));
    return [...info.animations, ...combines, ...Object.keys(map.animations).filter((name) => !info.animations.includes(name) && !combines.includes(name))];
};

// whether an animation the map shows plays: its generator is connected, or it's a combine drawn in the map
export const isPlaying = (map: NoteMap, info: MapData, name: string) => info.animations.includes(name) || name in map.combines;

// what an animation is called: a generator's name, a combine is named after what goes into it ("spring + cymbal") like
// the backend names it
export function animationLabel(map: NoteMap, name: string, seen: string[] = []): string {
    const combine = map.combines[name];
    if (!combine || seen.includes(name)) return name;
    return combine.inputs
        .map((input) => animationLabel(map, input, [...seen, name]))
        .filter((label) => label)
        .join(" + ");
}

// a key for a new combine, one no animation has
export function newCombineKey(map: NoteMap, info: MapData): string {
    let n = 1;
    while (`combine_${n}` in map.combines || `combine_${n}` in map.animations || info.animations.includes(`combine_${n}`)) n++;
    return `combine_${n}`;
}

// whether `inner` is `outer` or goes into it, a combine can't go into itself
export function goesInto(map: NoteMap, inner: string, outer: string): boolean {
    if (inner === outer) return true;
    return (map.combines[outer]?.inputs ?? []).some((input) => input in map.combines && goesInto(map, inner, input));
}

// true if `note` is wired to one of `objects`. with linked only (the node's `linked_only`) the map and its preview show
// only these, and the animations `isAnimationLinked` says are
export const isLinked = (map: NoteMap, objects: string[], note: number) => objects.some((o) => map.objects[o]?.includes(note));
// an animation going into a combine that's linked is linked too
export const isAnimationLinked = (map: NoteMap, objects: string[], name: string, seen: string[] = []): boolean => (map.animations[name] ?? []).some((o) => objects.includes(o)) || Object.entries(map.combines).some(([key, combine]) => combine.inputs.includes(name) && !seen.includes(key) && isAnimationLinked(map, objects, key, [...seen, name]));

// a stored map as given, anything that isn't one is empty. `animations()` gives the wires of the animations it doesn't
// have (no wires, going into no combine), so a generator connected after the map was drawn plays on every object
export function readMap(value: any, animations: () => Record<string, string[]> = () => ({})): NoteMap {
    const objects: Record<string, number[]> = {};
    for (const [name, notes] of Object.entries(value?.objects ?? {})) {
        if (Array.isArray(notes)) objects[name] = notesIn(notes);
    }
    const combines: Record<string, MapCombine> = {};
    if (value?.combines && typeof value.combines === "object" && !Array.isArray(value.combines)) {
        for (const [key, combine] of Object.entries<any>(value.combines)) {
            const inputs = list(combine?.inputs).filter((i) => typeof i === "string");
            combines[key] = typeof combine?.animation_overlap === "string" ? { inputs, animation_overlap: combine.animation_overlap } : { inputs };
        }
    }
    const wired = Object.fromEntries(Object.entries(animations()).filter(([name]) => !Object.values(combines).some((c) => c.inputs.includes(name))));
    if (value?.animations && typeof value.animations === "object" && !Array.isArray(value.animations)) {
        for (const [name, on] of Object.entries(value.animations)) wired[name] = list(on).filter((o) => typeof o === "string");
    }
    return { objects, notes: notesIn(value?.notes), animations: wired, combines };
}
