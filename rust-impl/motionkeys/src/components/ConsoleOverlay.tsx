import { useEffect, useState } from "react";
import { ConsoleEntry, revealNode, useConsoleEntries, useConsoleSettings } from "../utils/console";

// how long a line takes to fade out once its time is up
const FADE_OUT_MS = 600;
// the height of a row of buttons in the canvas' top left corner (NodeGraph.tsx), and the gap above the first
const CORNER_ROW = 28;
const CORNER_TOP = 8;

// a run's line saying it started, and the one saying it finished (which has how long it took). see run_instance in
// src-tauri/src/graph/execute.rs
const runStarted = (entry: ConsoleEntry) => entry.source === "run" && entry.level === "info" && entry.ms == null;
const runFinished = (entry: ConsoleEntry) => entry.source === "run" && entry.ms != null;

// the last `count` lines, a run's start left out once its tab's run has finished: the finished line takes its place
function lastLines(entries: ConsoleEntry[], count: number): ConsoleEntry[] {
    const lines: ConsoleEntry[] = [];
    // tabs with a finished line after this point, whose start is still to come going back
    const finished = new Set<string | null>();
    for (let i = entries.length - 1; i >= 0 && lines.length < count; i--) {
        const entry = entries[i];
        if (runFinished(entry)) finished.add(entry.tab);
        else if (runStarted(entry) && finished.delete(entry.tab)) continue;
        lines.unshift(entry);
    }
    return lines;
}

// the console's last lines drawn straight on the canvas, in its top left corner past the left docked panels. only a line
// about a node takes the mouse, clicking it selects and frames the node. `rows`: rows of buttons already in the corner
function ConsoleOverlay({ rows }: { rows: number }) {
    const entries = useConsoleEntries();
    const settings = useConsoleSettings();

    // lines go after the fade time (none with 0), fading out first. redrawn when the next one starts or ends fading
    const [now, setNow] = useState(() => Date.now());
    const fadeMs = Math.max(0, settings.fade_seconds) * 1000;
    const count = Math.max(0, settings.inline_lines);
    const recent = settings.overlay && count > 0 ? lastLines(entries, count) : [];
    const lines = fadeMs > 0 ? recent.filter((entry) => now - entry.time < fadeMs) : recent;
    const fading = (entry: ConsoleEntry) => fadeMs > 0 && now - entry.time >= fadeMs - FADE_OUT_MS;
    useEffect(() => {
        if (fadeMs === 0 || lines.length === 0) return;
        const next = Math.min(...lines.map((entry) => entry.time + fadeMs - (fading(entry) ? 0 : FADE_OUT_MS)));
        const timer = setTimeout(() => setNow(Date.now()), Math.max(0, next - Date.now()) + 16);
        return () => clearTimeout(timer);
    }, [lines, fadeMs, now]);

    if (lines.length === 0) return null;
    return (
        <div className="console-overlay" style={{ top: CORNER_TOP + rows * CORNER_ROW }}>
            {lines.map((entry) => (
                <div key={entry.id} className={`console-line console-${entry.level}${fading(entry) ? " console-line-out" : ""}`}>
                    {entry.node && entry.tab ? (
                        <span className="console-line-link" onClick={() => revealNode(entry.tab!, entry.node!)}>
                            {entry.message}
                        </span>
                    ) : (
                        entry.message
                    )}
                </div>
            ))}
        </div>
    );
}

export default ConsoleOverlay;
