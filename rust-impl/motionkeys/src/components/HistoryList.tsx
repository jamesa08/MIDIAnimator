import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { historyLabel } from "../utils/history";

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
    // braces matter, Chromium's scrollIntoView returns a promise that React would call as the cleanup
    useEffect(() => {
        currentRef.current?.scrollIntoView({ block: "nearest" });
    }, [history]);

    // row `i` is the project with the first `i` steps done
    // cut and paste rows also name the nodes, MCP steps' details are for the MCP client
    const rows = [{ key: "original", label: ORIGINAL, detail: "" }, ...history.entries.map((entry) => ({ key: String(entry.id), label: historyLabel(entry.op), detail: entry.source === "ui" ? entry.detail : "" }))];

    return (
        <div className="font-[Arial,sans-serif] text-xs p-1">
            {rows.map((row, i) => {
                const current = i === history.current;
                const undone = i > history.current;
                return (
                    <div key={row.key} ref={current ? currentRef : undefined} className={`h-5 px-1.5 flex items-center whitespace-nowrap ${current ? "bg-black text-white" : `hover:bg-zinc-100 ${undone ? "text-zinc-400" : ""}`}`} onClick={() => invoke("history_goto", { current: i })}>
                        <span className="flex-none">{row.label}</span>
                        {row.detail && <span className="ml-1.5 min-w-0 truncate text-zinc-400">{row.detail}</span>}
                    </div>
                );
            })}
        </div>
    );
}

export default HistoryList;
