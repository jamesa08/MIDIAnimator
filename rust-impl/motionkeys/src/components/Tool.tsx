import React, { useEffect, useRef, useState } from "react";
import { useStateContext } from "../contexts/StateContext";
import { invoke } from "@tauri-apps/api/core";
import { useOpenFile, useSaveTab } from "../utils/tabs";
import { Icon, IconName } from "../icons";
import { runCommand } from "../utils/keymap";

function Tool({ type, onClick }: { type: string; onClick?: () => void }) {
    const { backEndState } = useStateContext();
    const saveTab = useSaveTab();
    const openFile = useOpenFile();

    // after a write the run button shows how it went for a moment, a check or a warning
    const HOLD_MS = 1200;
    const write = backEndState?.last_write;
    const lastSeq = useRef(write?.seq);
    const [outcome, setOutcome] = useState<"done" | "failed" | null>(null);
    useEffect(() => {
        if (type != "run" || write == null || write.seq === lastSeq.current) return;
        lastSeq.current = write.seq;
        setOutcome(write.error == null && write.written > 0 ? "done" : "failed");
        const timer = window.setTimeout(() => setOutcome(null), HOLD_MS);
        return () => window.clearTimeout(timer);
    }, [type, write?.seq]);

    var icon;
    if (type == "run" && outcome == "done") {
        icon = (
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="tool-icon">
                <path strokeLinecap="round" strokeLinejoin="round" d="m4.5 12.75 6 6 9-13.5" />
            </svg>
        );
    } else if (type == "run" && outcome == "failed") {
        icon = (
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="tool-icon">
                <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z" />
            </svg>
        );
    } else if (type == "run") {
        icon = <Icon name="run" className="tool-icon" />;
    } else if (type == "save") {
        icon = <Icon name="save" className="tool-icon" />;
    } else if (type == "load") {
        icon = <Icon name="open" className="tool-icon" />;
    } else if (type == "collapse-left") {
        icon = <Icon name="hide_panels" className="tool-icon" />;
    } else if (type == "collapse-right") {
        icon = <Icon name="hide_panels" className="tool-icon flip" />;
    }

    return (
        <div
            className="toolbar-button"
            onClick={async () => {
                if (type == "run") {
                    if (backEndState.execution_paused) {
                        alert("Execution paused: Validate scene data first");
                        return;
                    }
                    invoke("execute_graph", { realtime: false });
                } else if (type == "save") {
                    try {
                        const path = await saveTab();
                        console.log("Saved to:", path);
                    } catch (error) {
                        if (error !== "Save cancelled") console.error("Save failed:", error);
                    }
                } else if (type == "load") {
                    openFile();
                }

                onClick?.();
            }}
        >
            {icon}
        </div>
    );
}

// a button that runs an app or area command, the same as its menu item. faded when it can't run now
export function CommandTool({ icon, command, disabled = false }: { icon: IconName; command: string; disabled?: boolean }) {
    return (
        <div className={`toolbar-button${disabled ? " disabled" : ""}`} data-command={command} onClick={() => runCommand(command)}>
            <Icon name={icon} className="tool-icon" />
        </div>
    );
}

export default Tool;
