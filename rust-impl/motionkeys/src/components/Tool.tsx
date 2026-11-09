import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useStateContext } from "../contexts/StateContext";
import { invoke } from "@tauri-apps/api/core";
import { useOpenFile, useSaveTab } from "../utils/tabs";
import { Icon, IconName } from "../icons";
import { formatCombo, runCommand, useKeymapData } from "../utils/keymap";

// a toolbar button's raised and pressed looks (index.css), kept from pointer events. webkit drops :hover for a frame after
// a click, which flashed the button flat between pressed and raised. a press let go outside the button still ends, and
// leaving the window (a save dialog) or the button fading forgets the mouse. `tip` is shown under the button while the
// mouse rests on it (ToolTip), the button puts `tip` from here among its children
export function useButtonTravel(disabled = false, tip: Tip | null = null) {
    const [hover, setHover] = useState(false);
    const [pressed, setPressed] = useState(false);
    // a click puts the tooltip away until the mouse leaves
    const [clicked, setClicked] = useState(false);
    const anchor = useRef<HTMLElement | null>(null);
    const tipShown = useTipDelay(hover && !pressed && !clicked && tip !== null);
    const reset = () => {
        setHover(false);
        setPressed(false);
        setClicked(false);
    };
    useEffect(() => {
        window.addEventListener("blur", reset);
        return () => window.removeEventListener("blur", reset);
    }, []);
    useEffect(() => {
        if (disabled) reset();
    }, [disabled]);
    useEffect(() => {
        if (!pressed) return;
        const release = () => setPressed(false);
        window.addEventListener("pointerup", release);
        return () => window.removeEventListener("pointerup", release);
    }, [pressed]);

    return {
        className: `${hover ? " hover" : ""}${hover && pressed ? " pressed" : ""}`,
        handlers: {
            onPointerEnter: (event: React.PointerEvent<HTMLElement>) => {
                anchor.current = event.currentTarget;
                setHover(true);
            },
            onPointerLeave: () => {
                setHover(false);
                setClicked(false);
            },
            onPointerDown: (event: React.PointerEvent) => {
                if (event.button !== 0) return;
                setHover(true);
                setPressed(true);
                setClicked(true);
            },
        },
        tip: tipShown && tip && anchor.current ? <ToolTip tip={tip} anchor={anchor.current} /> : null,
    };
}

// MARK: - Tooltips

// what a tooltip says: the button's name, and its key under that
export type Tip = { name: string; shortcut?: string };

// how long the mouse rests on a button before its tooltip shows (ms). moving on to the next button within TIP_FOLLOW of one
// going away shows that one's at once
const TIP_DELAY = 500;
const TIP_FOLLOW = 300;
let tipHiddenAt = -Infinity;

function useTipDelay(wanted: boolean): boolean {
    const [shown, setShown] = useState(false);
    const shownRef = useRef(false);
    useEffect(() => {
        if (!wanted) return;
        const timer = window.setTimeout(
            () => {
                shownRef.current = true;
                setShown(true);
            },
            performance.now() - tipHiddenAt < TIP_FOLLOW ? 0 : TIP_DELAY
        );
        return () => {
            window.clearTimeout(timer);
            if (shownRef.current) tipHiddenAt = performance.now();
            shownRef.current = false;
            setShown(false);
        };
    }, [wanted]);
    return shown;
}

// a button's name under it, kept inside the window
function ToolTip({ tip, anchor }: { tip: Tip; anchor: HTMLElement }) {
    const ref = useRef<HTMLDivElement>(null);
    const rect = anchor.getBoundingClientRect();
    const [left, setLeft] = useState(rect.left);
    useLayoutEffect(() => {
        const width = ref.current?.offsetWidth ?? 0;
        setLeft(Math.max(4, Math.min(rect.left, window.innerWidth - width - 4)));
    }, [tip.name, tip.shortcut, rect.left]);
    return createPortal(
        <div ref={ref} className="toolbar-tooltip" style={{ left, top: rect.bottom + 4 }}>
            <div>{tip.name}</div>
            {tip.shortcut && <div className="toolbar-tooltip-shortcut">Shortcut: {tip.shortcut}</div>}
        </div>,
        document.body
    );
}

// a command's name and its first key, the way a menu shows it ("⌘S"). an area's command is the node editor's
export function useCommandTip(command: string): Tip | null {
    const keymap = useKeymapData();
    const [scope, id] = command.startsWith("area.") ? ["node_editor", command.slice("area.".length)] : ["app", command];
    const found = keymap?.scopes.find((s) => s.id === scope)?.commands.find((c) => c.id === id);
    if (!keymap || !found) return null;
    return { name: found.name, shortcut: found.keys.length > 0 ? formatCombo(found.keys[0], keymap.platform) : undefined };
}

// the buttons that aren't a command, the command each one's name comes from or the name itself
const TOOL_COMMANDS: Record<string, string> = { save: "save", load: "open" };
const TOOL_LABELS: Record<string, string> = { run: "Run", "collapse-left": "Left Panels", "collapse-right": "Right Panels" };

function Tool({ type, onClick }: { type: string; onClick?: () => void }) {
    const commandTip = useCommandTip(TOOL_COMMANDS[type] ?? "");
    const travel = useButtonTravel(false, TOOL_LABELS[type] ? { name: TOOL_LABELS[type] } : commandTip);
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
            className={`toolbar-button${travel.className}`}
            {...travel.handlers}
            onClick={async () => {
                if (type == "run") {
                    if (backEndState.execution_paused) {
                        alert("Execution paused: Validate scene data first");
                        return;
                    }
                    // the tab the button was shown for, the backend doesn't run another one MCP switched to meanwhile
                    invoke("execute_graph", { realtime: false, tab: backEndState.active_tab }).catch((error) => console.error("Run failed:", error));
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
            {travel.tip}
        </div>
    );
}

// a button that runs an app or area command, the same as its menu item. faded when it can't run now, pressed in while
// what it turns on is on
export function CommandTool({ icon, command, disabled = false, active = false }: { icon: IconName; command: string; disabled?: boolean; active?: boolean }) {
    const travel = useButtonTravel(disabled, useCommandTip(command));
    return (
        <div className={`toolbar-button${disabled ? " disabled" : ""}${active ? " active" : ""}${travel.className}`} data-command={command} {...travel.handlers} onClick={() => runCommand(command)}>
            <Icon name={icon} className="tool-icon" />
            {travel.tip}
        </div>
    );
}

export default Tool;
