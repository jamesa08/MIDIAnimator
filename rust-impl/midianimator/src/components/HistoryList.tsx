import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

// row text for each undo step by the op that made it (Op::name in src-tauri/src/graph/ops.rs, and the MCP tools'
// ops). placeholders until the real copy is written, an op missing here shows its own name
const LABELS: Record<string, string> = {
    add_nodes: "Add Nodes",
    add_node: "Add Node",
    delete: "Delete",
    remove_node: "Remove Node",
    connect: "Connect",
    disconnect: "Disconnect",
    set_inputs: "Set Inputs",
    move: "Move",
    resize: "Resize",
    select: "Select",
    duplicate: "Duplicate",
    paste: "Paste",
    cut: "Cut",
    group: "Group",
    ungroup: "Ungroup",
    rename_socket: "Rename Socket",
    remove_socket: "Remove Socket",
    make_local: "Make Local",
    revert_group: "Revert to Built-in",
};
// the first row, the project before any step
const ORIGINAL = "Original";

// the undo history from the backend (HistoryInfo in src-tauri/src/graph/history.rs), the first `current` entries are done
type Entry = { id: number; op: string; source: "ui" | "mcp"; detail: string };
type History = { entries: Entry[]; current: number };

// the history panel: every step oldest first, the current one highlighted and the ones that can be redone dimmed.
// clicking a row undoes or redoes to it
function HistoryList() {
    const [history, setHistory] = useState<History>({ entries: [], current: 0 });

    useEffect(() => {
        invoke<History>("get_history").then(setHistory);
        const unlisten = listen<History>("history_changed", (event) => setHistory(event.payload));
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    // the current row stays in view
    const currentRef = useRef<HTMLDivElement>(null);
    useEffect(() => currentRef.current?.scrollIntoView({ block: "nearest" }), [history]);

    // row `i` is the project with the first `i` steps done
    const rows = [{ key: "original", label: ORIGINAL }, ...history.entries.map((entry) => ({ key: String(entry.id), label: LABELS[entry.op] ?? entry.op }))];

    return (
        <div className="font-[Arial,sans-serif] text-sm py-1">
            {rows.map((row, i) => {
                const current = i === history.current;
                const undone = i > history.current;
                return (
                    <div key={row.key} ref={current ? currentRef : undefined} className={`h-6 px-2 flex items-center ${current ? "bg-black text-white" : `hover:bg-zinc-100 ${undone ? "text-zinc-400" : ""}`}`} onClick={() => invoke("history_goto", { current: i })}>
                        {row.label}
                    </div>
                );
            })}
        </div>
    );
}

export default HistoryList;
