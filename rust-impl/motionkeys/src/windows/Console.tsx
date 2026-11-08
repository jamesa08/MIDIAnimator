import { MouseEvent, ReactNode, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { VList, VListHandle } from "virtua";
import { useStateContext } from "../contexts/StateContext";
import { useStateSync } from "../utils/graphOps";
import { isTextField, useKeymap, useModal } from "../utils/keymap";
import { usePersistedSet, usePersistedState } from "../utils/persist";
import { useNodeSpecs } from "../utils/nodeEntries";
import { ConsoleEntry, LEVELS, Level, NodeTiming, SOURCES, Source, duration, revealNode, useConsoleEntries, useConsoleSettings, useRunTimings } from "../utils/console";
import { TRAFFIC_LIGHTS_WIDTH } from "./Graph";

// every row is this tall, so the list never has to measure one
const ROW_HEIGHT = 18;
// the toolbar's copy button asks the log for its rows
const COPY_EVENT = "console-copy-rows";
const LEVEL_NAMES: Record<Level, string> = { error: "Error", warn: "Warn", info: "Info", debug: "Debug" };
const SOURCE_NAMES: Record<Source, string> = { run: "Run", node: "Node", bridge: "3D App", app: "App", rust: "Rust", js: "JS" };

// the time of day an entry came, to the millisecond
function clock(time: number) {
    const date = new Date(time);
    const pad = (n: number, width = 2) => String(n).padStart(width, "0");
    return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds(), 3)}`;
}

// an entry as a line of plain text, its parts tab separated
const entryText = (entry: ConsoleEntry) => [clock(entry.time), LEVEL_NAMES[entry.level], SOURCE_NAMES[entry.source], entry.message, entry.ms != null ? duration(entry.ms) : "", entry.node ?? ""].join("\t").trimEnd();

// puts text on the clipboard through a copy event, the clipboard api is the fallback
function copyText(text: string) {
    const onCopy = (event: ClipboardEvent) => {
        event.clipboardData?.setData("text/plain", text);
        event.preventDefault();
        event.stopImmediatePropagation();
    };
    document.addEventListener("copy", onCopy, true);
    const copied = document.execCommand("copy");
    document.removeEventListener("copy", onCopy, true);
    if (!copied) navigator.clipboard?.writeText(text).catch(() => {});
}

// buttons joined in one box, like the graph window's seconds and frames. `on` for the ones that are on
function Segmented({ items }: { items: { name: string; on: boolean; onClick: () => void }[] }) {
    return (
        <div className="flex flex-none border border-black">
            {items.map((item, i) => (
                <button key={item.name} className={`h-6 px-2 ${i > 0 ? "border-l border-black" : ""} ${item.on ? "bg-black text-white" : "hover:bg-zinc-100"}`} onClick={item.onClick}>
                    {item.name}
                </button>
            ))}
        </div>
    );
}

function Button({ onClick, children }: { onClick: () => void; children: ReactNode }) {
    return (
        <button className="flex-none h-6 px-2 border border-black hover:bg-zinc-100" onClick={onClick}>
            {children}
        </button>
    );
}

// `text` with every match of `query` marked
function Highlight({ text, query, current }: { text: string; query: string; current: boolean }) {
    if (!query) return <>{text}</>;
    const lower = text.toLowerCase();
    const q = query.toLowerCase();
    const parts: ReactNode[] = [];
    let at = 0;
    for (let i = lower.indexOf(q); i >= 0; i = lower.indexOf(q, i + q.length)) {
        parts.push(text.slice(at, i));
        parts.push(
            <mark key={i} className={`console-match${current ? " console-match-current" : ""}`}>
                {text.slice(i, i + q.length)}
            </mark>
        );
        at = i + q.length;
    }
    parts.push(text.slice(at));
    return <>{parts}</>;
}

// MARK: - Log

// the console's entries, filtered by level and source, newest at the bottom. it follows new ones while scrolled to the
// bottom, scrolling up stops that until it's back at the bottom
function Log({ levels, sources, findOpen, setFindOpen }: { levels: Set<string>; sources: Set<string>; findOpen: boolean; setFindOpen: (open: boolean) => void }) {
    const entries = useConsoleEntries();
    const visible = useMemo(() => entries.filter((entry) => levels.has(entry.level) && sources.has(entry.source)), [entries, levels, sources]);

    // new rows come into view while following, the bottom of the list follows
    const list = useRef<VListHandle>(null);
    const [follow, setFollow] = useState(true);
    useLayoutEffect(() => {
        if (follow && visible.length > 0) list.current?.scrollToIndex(visible.length - 1, { align: "end" });
    }, [visible, follow]);
    const onScroll = (offset: number) => {
        const handle = list.current;
        if (handle) setFollow(offset + handle.viewportSize >= handle.scrollSize - 2);
    };

    // rows are picked like files: a click picks one, cmd adds or takes away one, shift picks the ones between
    const [selected, setSelected] = useState<Set<number>>(new Set());
    const anchor = useRef<number | null>(null);
    const select = (index: number, event: MouseEvent) => {
        const id = visible[index].id;
        const from = anchor.current === null ? -1 : visible.findIndex((entry) => entry.id === anchor.current);
        if (event.shiftKey && from >= 0) {
            const [a, b] = from < index ? [from, index] : [index, from];
            setSelected(new Set(visible.slice(a, b + 1).map((entry) => entry.id)));
            return;
        }
        anchor.current = id;
        if (event.metaKey || event.ctrlKey)
            setSelected((prev) => {
                const next = new Set(prev);
                if (next.has(id)) next.delete(id);
                else next.add(id);
                return next;
            });
        else setSelected(new Set([id]));
    };

    // the picked rows as text, every row shown when none are picked. copy from the edit menu copies them too
    const rowsText = () => {
        const picked = visible.filter((entry) => selected.has(entry.id));
        return (picked.length > 0 ? picked : visible).map(entryText).join("\n");
    };
    const rowsTextRef = useRef(rowsText);
    rowsTextRef.current = rowsText;
    useEffect(() => {
        const copyRows = () => copyText(rowsTextRef.current());
        const onCopy = (event: ClipboardEvent) => {
            if (isTextField(document.activeElement)) return;
            event.clipboardData?.setData("text/plain", rowsTextRef.current());
            event.preventDefault();
        };
        document.addEventListener("copy", onCopy);
        window.addEventListener(COPY_EVENT, copyRows);
        return () => {
            document.removeEventListener("copy", onCopy);
            window.removeEventListener(COPY_EVENT, copyRows);
        };
    }, []);

    // MARK: Find

    const [query, setQuery] = useState("");
    const [focused, setFocused] = useState(false);
    const input = useRef<HTMLInputElement>(null);
    const matches = useMemo(() => {
        const q = query.toLowerCase();
        if (!findOpen || !q) return [];
        return visible.flatMap((entry, i) => (entry.message.toLowerCase().includes(q) || entry.node?.toLowerCase().includes(q) ? [i] : []));
    }, [visible, query, findOpen]);
    // the match shown, by entry id so new rows don't move it
    const [current, setCurrent] = useState<number | null>(null);
    const at = matches.findIndex((i) => visible[i].id === current);

    // shows a match in the middle of the list, the list stops following
    const show = (index: number) => {
        setCurrent(visible[index].id);
        setFollow(false);
        list.current?.scrollToIndex(index, { align: "center" });
    };
    const step = (by: number) => {
        if (matches.length === 0) return;
        const from = at < 0 ? (by > 0 ? -1 : 0) : at;
        show(matches[(from + by + matches.length) % matches.length]);
    };
    // a new search shows the newest match
    useEffect(() => {
        if (matches.length > 0) show(matches[matches.length - 1]);
        else setCurrent(null);
    }, [query]);

    // opened with the find key, focused and its text picked to type over
    useEffect(() => {
        if (findOpen) {
            input.current?.focus();
            input.current?.select();
        } else {
            setQuery("");
            setFocused(false);
        }
    }, [findOpen]);
    useModal("console_find", { next: () => step(1), previous: () => step(-1), cancel: () => setFindOpen(false) }, findOpen && focused, { passthrough: true });

    return (
        <div className="card bg-white flex-auto min-h-0 flex flex-col">
            {findOpen && (
                <div className="flex-none flex items-center gap-2 px-2 h-8 border-b border-[var(--chrome-line)]">
                    <input ref={input} className="w-56 h-6 px-1 border border-black bg-white" value={query} onChange={(e) => setQuery(e.target.value)} onFocus={() => setFocused(true)} onBlur={() => setFocused(false)} />
                    {query && <span className="tabular-nums">{`${at + 1}/${matches.length}`}</span>}
                </div>
            )}
            <VList ref={list} data={visible} itemSize={ROW_HEIGHT} onScroll={onScroll} className="flex-auto min-h-0">
                {(entry, index) => (
                    <div key={entry.id} className={`flex items-center gap-3 px-2 whitespace-nowrap${selected.has(entry.id) ? " console-row-selected" : ""}`} style={{ height: ROW_HEIGHT }} onMouseDown={(event) => select(index, event)}>
                        <span className="flex-none w-[76px] tabular-nums">{clock(entry.time)}</span>
                        <span className={`flex-none w-9 console-${entry.level}`}>{LEVEL_NAMES[entry.level]}</span>
                        <span className="flex-none w-12">{SOURCE_NAMES[entry.source]}</span>
                        <span className={`flex-auto min-w-0 truncate console-${entry.level}`}>
                            <Highlight text={entry.message} query={findOpen ? query : ""} current={entry.id === current} />
                        </span>
                        {entry.ms != null && <span className="flex-none tabular-nums">{duration(entry.ms)}</span>}
                        {entry.node && entry.tab && (
                            <button className="flex-none max-w-48 truncate hover:underline" onClick={() => revealNode(entry.tab!, entry.node!)}>
                                <Highlight text={entry.node} query={findOpen ? query : ""} current={entry.id === current} />
                            </button>
                        )}
                    </div>
                )}
            </VList>
        </div>
    );
}

// MARK: - Run

type SortKey = "label" | "type" | "cached" | "ms" | "status" | "group" | "zone" | "iterations";

const COLUMNS: { key: SortKey; name: string; right?: boolean }[] = [
    { key: "label", name: "Node" },
    { key: "type", name: "Type" },
    { key: "cached", name: "Ran" },
    { key: "ms", name: "Time", right: true },
    { key: "status", name: "Status" },
    { key: "group", name: "Group" },
    { key: "zone", name: "Zone" },
    { key: "iterations", name: "Iterations", right: true },
];
const STATUS_NAMES: Record<NodeTiming["status"], string> = { ok: "Ok", failed: "Failed", waiting: "Waiting", skipped: "Skipped" };
const STATUS_CLASSES: Record<NodeTiming["status"], string> = { ok: "", failed: "console-error", waiting: "console-debug", skipped: "console-debug" };

// the last run of the tab on screen, node by node, slowest first. clicking a row frames its node
function Run({ tab }: { tab: string }) {
    const timings = useRunTimings(tab);

    // types by their names, groups and zones by their nodes' labels
    const specs = useNodeSpecs();
    const nodes = timings?.nodes ?? [];
    const labels = useMemo(() => new Map(nodes.map((t) => [t.node, t.label])), [nodes]);
    const typeName = (t: NodeTiming) => specs.find((spec: any) => spec.id === t.node_type)?.name ?? t.node_type;
    const pathName = (path: string | null) => (path ? (labels.get(path) ?? path) : "");

    const [sort, setSort] = usePersistedState<{ key: SortKey; desc: boolean }>("console.run_sort", { key: "ms", desc: true });
    const value = (t: NodeTiming, key: SortKey): string | number => {
        switch (key) {
            case "label":
                return t.label;
            case "type":
                return typeName(t);
            case "cached":
                return t.cached ? 1 : 0;
            case "ms":
                return t.ms;
            case "status":
                return STATUS_NAMES[t.status];
            case "group":
                return pathName(t.group);
            case "zone":
                return pathName(t.zone);
            case "iterations":
                return t.iterations ?? -1;
        }
    };
    const rows = [...nodes].sort((a, b) => {
        const [x, y] = [value(a, sort.key), value(b, sort.key)];
        const order = typeof x === "number" && typeof y === "number" ? x - y : String(x).localeCompare(String(y));
        return sort.desc ? -order : order;
    });
    // a column clicked again turns around, numbers start biggest first
    const sortBy = (key: SortKey) => setSort((prev) => (prev.key === key ? { key, desc: !prev.desc } : { key, desc: key === "ms" || key === "iterations" }));

    return (
        <div className="card bg-white flex-auto min-h-0 overflow-auto">
            <table className="w-full border-collapse">
                <thead>
                    <tr>
                        {COLUMNS.map((column) => (
                            <th key={column.key} className={`sticky top-0 bg-white font-normal px-2 h-6 border-b border-[var(--chrome-line)] cursor-default whitespace-nowrap ${column.right ? "text-right" : "text-left"}`} onClick={() => sortBy(column.key)}>
                                {column.name}
                                {sort.key === column.key && (sort.desc ? " ▼" : " ▲")}
                            </th>
                        ))}
                    </tr>
                </thead>
                <tbody>
                    {rows.map((t) => (
                        <tr key={t.node} className="hover:bg-zinc-100 whitespace-nowrap" style={{ height: ROW_HEIGHT }} onClick={() => revealNode(tab, t.node)}>
                            <td className="px-2">{t.label}</td>
                            <td className="px-2">{typeName(t)}</td>
                            <td className={`px-2 ${t.cached ? "console-debug" : ""}`}>{t.cached ? "Cached" : "Ran"}</td>
                            <td className="px-2 text-right tabular-nums">{duration(t.ms)}</td>
                            <td className={`px-2 ${STATUS_CLASSES[t.status]}`}>{STATUS_NAMES[t.status]}</td>
                            <td className="px-2">{pathName(t.group)}</td>
                            <td className="px-2">{pathName(t.zone)}</td>
                            <td className="px-2 text-right tabular-nums">{t.iterations ?? ""}</td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
    );
}

// MARK: - Window

// the console window: the console's entries with filters and find, or the last run's timings
function Console() {
    useStateSync();
    const { backEndState: state } = useStateContext();
    const settings = useConsoleSettings();
    const [view, setView] = usePersistedState<"log" | "run">("console.view", "log");
    const [levels, setLevels] = usePersistedSet("console.levels", LEVELS);
    const [sources, setSources] = usePersistedSet("console.sources", SOURCES);
    const [findOpen, setFindOpen] = useState(false);

    const toggle = (set: (update: (prev: Set<string>) => Set<string>) => void, item: string) =>
        set((prev) => {
            const next = new Set(prev);
            if (next.has(item)) next.delete(item);
            else next.add(item);
            return next;
        });
    const setDebug = (on: boolean) => invoke("set_setting", { path: "console.debug", value: on }).catch((e) => console.error(`Error saving setting console.debug: ${e}`));

    // the find key opens find on the log, escape closes it
    useKeymap("console", {
        find: () => {
            setView("log");
            setFindOpen(true);
        },
        close_find: () => setFindOpen(false),
    });

    return (
        <div data-live-resize="window" data-keymap-area="console" className="console-window w-screen h-screen flex flex-col overflow-hidden select-none font-[Arial,sans-serif] text-xs">
            <div data-tauri-drag-region className="toolbar flex-none h-9 flex items-center gap-1.5 pr-1.5 border-b border-[var(--chrome-line)] overflow-hidden" style={{ paddingLeft: TRAFFIC_LIGHTS_WIDTH }}>
                <Segmented
                    items={[
                        { name: "Console", on: view === "log", onClick: () => setView("log") },
                        { name: "Run", on: view === "run", onClick: () => setView("run") },
                    ]}
                />
                {view === "log" && (
                    <>
                        <div className="spacer h-5 w-[1px] mx-1 flex-none" />
                        <Segmented items={LEVELS.map((level) => ({ name: LEVEL_NAMES[level], on: levels.has(level), onClick: () => toggle(setLevels, level) }))} />
                        <Segmented items={SOURCES.map((source) => ({ name: SOURCE_NAMES[source], on: sources.has(source), onClick: () => toggle(setSources, source) }))} />
                        <Segmented items={[{ name: "Debug", on: settings.debug, onClick: () => setDebug(!settings.debug) }]} />
                        <div className="flex-auto" />
                        <Button onClick={() => window.dispatchEvent(new Event(COPY_EVENT))}>Copy</Button>
                        <Button onClick={() => invoke("console_clear").catch((e) => console.error(`Error clearing the console: ${e}`))}>Clear</Button>
                    </>
                )}
            </div>
            <div className="flex-auto min-h-0 flex flex-col p-1">{view === "log" ? <Log levels={levels} sources={sources} findOpen={findOpen} setFindOpen={setFindOpen} /> : <Run tab={state.active_tab ?? ""} />}</div>
        </div>
    );
}

export default Console;
