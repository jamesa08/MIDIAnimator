import { useEffect, useState, useCallback } from "react";
import { useStore } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { useNodeSpec } from "../utils/nodeEntries";
import { useSetInputs } from "../utils/graphOps";

// the note list as text: what was typed in as it is, a list set another way (MCP) comma separated.
// the node reads the numbers out of it when it runs (note_list_entries in executors/animation.rs)
export const noteNumbersText = (value: any): string => (typeof value === "string" ? value : Array.isArray(value) ? value.join(", ") : "");

function assign_notes_to_objects({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const nodeData = useNodeSpec("assign_notes_to_objects");
    const [name, setName] = useState(data.inputs?.object_group_name || "");

    useEffect(() => {
        setName(data.inputs?.object_group_name || "");
    }, [data.inputs?.object_group_name]);

    const handleUpdate = useCallback(() => {
        setInputs(id, { object_group_name: name });
    }, [id, name, setInputs]);

    // the note list, typed in unless a connection gives it. stored edges are reversed, `source` is the node taking the value
    const notesConnected = useStore((s) => s.edges.some((e) => e.source === id && e.sourceHandle === "note_numbers"));
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
        <>
            <div>
                <input type="text" className="node-field border border-gray-400 rounded px-2 py-1" placeholder="Object Group Name" value={name} onChange={(e) => setName(e.target.value)} onBlur={handleUpdate} />
            </div>
        </>
    );

    const noteNumbersComponent = !notesConnected && (
        <div>
            <input type="text" className="node-field border border-gray-400 rounded px-2 py-1" value={notesText} onChange={(e) => setNotesText(e.target.value)} onBlur={handleNotesUpdate} />
        </div>
    );

    const uiInject = {
        object_group_name: objectGroupNameComponent,
        note_numbers: noteNumbersComponent,
    };

    const hiddenHandles = {};

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default assign_notes_to_objects;
