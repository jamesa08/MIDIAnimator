// the note map of Assign Notes to Objects as data: what it shows, read from the node's inputs and its last run. no
// components here, the node's preview (NoteMapPreview.tsx) and the map (NoteMapView.tsx) both read it. keep in sync with
// assign_notes_to_objects and NoteMap in src-tauri/src/graph/executors/animation.rs

// each object's notes, and notes added that nothing is connected to yet
export type NoteMap = { objects: Record<string, number[]>; notes: number[] };
// what the map shows from the node's last run: its object group's objects in name order, the MIDI's notes, and each
// object's notes in the object map it gave (what the rules gave, in rules mode)
export type MapData = { objects: string[]; midiNotes: number[]; assigned: Record<string, number[]> };

export const byNumber = (a: number, b: number) => a - b;
export const unique = (notes: number[]) => [...new Set(notes)].sort(byNumber);
const list = (value: any): any[] => (Array.isArray(value) ? value : []);

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
    const assigned: Record<string, number[]> = {};
    for (const [name, animations] of Object.entries(results?.object_map?.objects ?? {})) {
        assigned[name] = unique(
            Object.values(animations ?? {})
                .flatMap(list)
                .filter(Number.isInteger)
        );
    }
    return { objects, midiNotes, assigned };
}

// what the map shows: the rules' result in rules mode, the stored map (`noteMap`, the node's note_map) in map mode, and
// every note in it in order. the node's preview (NoteMapPreview.tsx) shows the same
export function shownMap(mode: string, info: MapData, noteMap: any): { map: NoteMap; notes: number[] } {
    let map: NoteMap;
    if (mode !== "map") {
        map = readMap({ objects: info.assigned });
        // padded notes the MIDI doesn't play stay in the map once it's edited, even with nothing connected
        map.notes = Object.values(map.objects)
            .flat()
            .filter((n) => !info.midiNotes.includes(n));
    } else {
        map = readMap(noteMap);
    }
    const notes = unique([...info.midiNotes, ...map.notes, ...info.objects.flatMap((o) => map.objects[o] ?? [])]);
    return { map, notes };
}

// a stored map as given, anything that isn't one is empty
export function readMap(value: any): NoteMap {
    const objects: Record<string, number[]> = {};
    for (const [name, notes] of Object.entries(value?.objects ?? {})) {
        if (Array.isArray(notes)) objects[name] = notes.filter((n) => Number.isInteger(n));
    }
    return { objects, notes: Array.isArray(value?.notes) ? value.notes.filter((n: any) => Number.isInteger(n)) : [] };
}
