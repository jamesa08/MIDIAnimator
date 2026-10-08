import { useEffect, useState, useCallback } from "react";
import { useStore, useUpdateNodeInternals } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { useNodeSpec } from "../utils/nodeEntries";
import { useSetInputs } from "../utils/graphOps";
import { useGroupContext } from "../contexts/GroupContext";
import NoteMapPreview from "../components/nodegraph/NoteMapPreview";

// the note list as text: what was typed in as it is, a list set another way (MCP) comma separated.
// the node reads the numbers out of it when it runs (note_list_entries in executors/animation.rs)
export const noteNumbersText = (value: any): string => (typeof value === "string" ? value : Array.isArray(value) ? value.join(", ") : "");

// MIDI note number to name like the old add-on, 60 = C3 (note_to_name in src-tauri/src/utils/mod.rs)
const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
export const noteToName = (note: number): string => `${NOTE_NAMES[note % 12]}${Math.floor(note / 12) - 2}`;

function assign_notes_to_objects({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const { openGroup } = useGroupContext();
    const preview = data === "preview" || data?.preview === true;
    const { backEndState: state } = useStateContext();

    const nodeData = useNodeSpec("assign_notes_to_objects");

    // the object groups the node ran with, like Keyframes from Object. a mistyped connection can hand over anything
    const objectGroups: any[] = Array.isArray(state?.executed_inputs?.[id]?.object_groups) ? state.executed_inputs[id].object_groups : [];
    const objectGroupNames: string[] = objectGroups.map((g: any) => g?.name);
    const selectedGroupName: string = data.inputs?.object_group_name || objectGroupNames[0] || "";

    // the first group is picked until one is, filled in automatically, not an undo step
    useEffect(() => {
        if (!preview && selectedGroupName && selectedGroupName !== data.inputs?.object_group_name) {
            setInputs(id, { object_group_name: selectedGroupName }, { commitToHistory: false });
        }
    }, [selectedGroupName]);

    // the note list, typed in unless a connection gives it. stored edges are reversed, `source` is the node taking the value
    const notesConnected = useStore((s) => s.edges.some((e) => e.source === id && e.sourceHandle === "note_numbers"));
    const mapMode = data.inputs?.mode === "map";
    const [notesText, setNotesText] = useState(noteNumbersText(data.inputs?.note_numbers));

    useEffect(() => {
        setNotesText(noteNumbersText(data.inputs?.note_numbers));
    }, [data.inputs?.note_numbers]);

    // kept as typed, emptied unsets it
    const handleNotesUpdate = useCallback(() => {
        if (notesText !== noteNumbersText(data.inputs?.note_numbers)) {
            setInputs(id, { note_numbers: notesText === "" ? null : notesText });
        }
    }, [id, notesText, data.inputs?.note_numbers, setInputs]);

    const objectGroupNameComponent = (
        <select className="node-field nodrag nopan" value={selectedGroupName} onChange={(e) => setInputs(id, { object_group_name: e.target.value })}>
            {objectGroupNames.length > 0 ? (
                objectGroupNames.map((name, i) => (
                    <option key={i} value={name}>
                        {name}
                    </option>
                ))
            ) : (
                <option value="">No ObjectGroup names found</option>
            )}
        </select>
    );

    // rules work out each object's notes, map takes them from the note map (Tab, or the button in the header)
    const modeComponent = (
        <>
            <div className="node-field">Mode</div>
            <select className="node-field nodrag nopan" value={data.inputs?.mode ?? "rules"} onChange={(e) => setInputs(id, { mode: e.target.value })}>
                <option value="rules">Rules</option>
                <option value="map">Map</option>
            </select>
        </>
    );

    const noteNumbersComponent = !notesConnected && !mapMode && (
        <div>
            <input type="text" className="node-field border border-gray-400 rounded px-2 py-1" value={notesText} onChange={(e) => setNotesText(e.target.value)} onBlur={handleNotesUpdate} />
        </div>
    );

    const openButton = preview ? null : (
        <button
            className="group-open nodrag nopan"
            onClick={(event) => {
                event.stopPropagation();
                openGroup(id);
            }}
        >
            ⧉
        </button>
    );

    const uiInject = {
        object_group_name: objectGroupNameComponent,
        mode: modeComponent,
        note_numbers: noteNumbersComponent,
    };

    // map mode doesn't use the note list, it's hidden unless something is connected to it (its edge needs the socket)
    const hideNoteNumbers = mapMode && !notesConnected;
    const updateNodeInternals = useUpdateNodeInternals();
    useEffect(() => {
        if (!preview) updateNodeInternals(id);
    }, [id, hideNoteNumbers, preview, updateNodeInternals]);

    // the group and the mode are dropdowns, the note map is drawn with Tab (or the button in the header)
    const hiddenHandles = {
        note_numbers: hideNoteNumbers,
        object_group_name: true,
        mode: true,
        note_map: true,
        map_layout: true,
    };

    // the note map, small, under the inputs
    return (
        <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} headerExtra={openButton}>
            <NoteMapPreview inputs={state?.executed_inputs?.[id]} results={state?.executed_results?.[id]} nodeInputs={data?.inputs} />
        </BaseNode>
    );
}

export default assign_notes_to_objects;
