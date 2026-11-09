import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { useStateContext } from "../contexts/StateContext";
import { queued } from "../utils/graphOps";
import { historyLabel } from "../utils/history";

// the first row, the project before any step
const ORIGINAL = "Original";

// the undo history from the backend (HistoryInfo in src-tauri/src/graph/history.rs), the first `current` entries are
// done. `tab` is whose it is, `rev` how new (it only goes up)
type Entry = { id: number; op: string; source: "ui" | "mcp"; detail: string };
type History = { tab: string; rev: number; entries: Entry[]; current: number };

// the history panel: every step oldest first, the current one highlighted and the ones that can be redone dimmed.
// clicking a row undoes or redoes to it
function HistoryList() {
    const { backEndState } = useStateContext();
    const tab: string = backEndState.active_tab ?? "";
    const [history, setHistory] = useState<History>({ tab: "", rev: 0, entries: [], current: 0 });

    // only the tab on screen's history is kept, and only a newer one replaces it (they can come in out of order)
    const tabRef = useRef(tab);
    tabRef.current = tab;
    const take = (next: History) => setHistory((prev) => (next.tab !== tabRef.current || (next.tab === prev.tab && next.rev < prev.rev) ? prev : next));
    useEffect(() => {
        const unlisten = listen<History>("history_changed", (event) => take(event.payload));
        return () => {
            unlisten.then((f) => f());
        };
    }, []);
    // another tab shown, its history is fetched
    useEffect(() => {
        if (tab) invoke<History>("get_history", { tab }).then(take);
    }, [tab]);
    // until the shown tab's history is in, nothing of another tab's is shown or clicked
    const shown = history.tab === tab ? history : { tab, rev: 0, entries: [], current: 0 };

    // the current row stays in view
    const currentRef = useRef<HTMLDivElement>(null);
    // braces matter, Chromium's scrollIntoView returns a promise that React would call as the cleanup
    useEffect(() => {
        currentRef.current?.scrollIntoView({ block: "nearest" });
    }, [shown]);

    // row `i` is the project with the first `i` steps done, it names the step it ends with so a history that changed
    // since goes to that step, or isn't changed if the step is gone
    const goto = (i: number) => {
        // not the rows shown while the tab's history is still on its way, "Original" would undo all of it
        if (history.tab !== tab) return;
        const entry = i > 0 ? (shown.entries[i - 1]?.id ?? null) : null;
        queued(tab, () => invoke("history_goto", { tab, current: i, entry })).catch((e) => console.error(`history_goto: ${e}`));
    };

    // cut and paste rows also name the nodes, MCP steps' details are for the MCP client
    const rows = [{ key: "original", label: ORIGINAL, detail: "" }, ...shown.entries.map((entry) => ({ key: String(entry.id), label: historyLabel(entry.op), detail: entry.source === "ui" ? entry.detail : "" }))];

    return (
        <div className="font-[Arial,sans-serif] text-xs p-1">
            {rows.map((row, i) => {
                const current = i === shown.current;
                const undone = i > shown.current;
                return (
                    <div key={row.key} ref={current ? currentRef : undefined} className={`h-5 px-1.5 flex items-center whitespace-nowrap ${current ? "bg-black text-white" : `hover:bg-zinc-100 ${undone ? "text-zinc-400" : ""}`}`} onClick={() => goto(i)}>
                        <span className="flex-none">{row.label}</span>
                        {row.detail && <span className="ml-1.5 min-w-0 truncate text-zinc-400">{row.detail}</span>}
                    </div>
                );
            })}
        </div>
    );
}

export default HistoryList;
