import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { useStateContext } from "../contexts/StateContext";
import { Keymap, MOUSE_BUTTONS, formatCombo, splitCombo, useKeymapData } from "../utils/keymap";
import { Hint, showStatus, useStatusHints, useStatusMessage } from "../utils/status";

const plural = (count: number, word: string) => `${count.toLocaleString()} ${word}${count === 1 ? "" : "s"}`;

// what the last write to Blender did, see LastWrite in src-tauri/src/state/mod.rs
export function lastWriteMessage(write: any): string | undefined {
    if (!write) return undefined;
    if (write.error != null) return "Scene Writer failed";
    if (write.written === 0) return "Nothing to write";
    return `Wrote ${plural(write.keyframes, "keyframe")} to ${plural(write.objects, "object")} in ${write.ms} ms`;
}

// a mouse with the pressed button filled in, like blender's
function MouseIcon({ button }: { button: number }) {
    const clip = `status-mouse-${button}`;
    return (
        <svg width="8" height="11" viewBox="0 0 8 11" className="inline-block">
            <clipPath id={clip}>
                <rect x="0.5" y="0.5" width="7" height="10" rx="3" />
            </clipPath>
            {button === 0 && <rect x="0" y="0" width="4" height="4.5" fill="currentColor" clipPath={`url(#${clip})`} />}
            {button === 2 && <rect x="4" y="0" width="4" height="4.5" fill="currentColor" clipPath={`url(#${clip})`} />}
            {button === 1 && <rect x="3" y="1.5" width="2" height="2.5" fill="currentColor" />}
            <rect x="0.5" y="0.5" width="7" height="10" rx="3" fill="none" stroke="currentColor" />
            <path d="M0.5 4.5H7.5M4 0.5V4.5" stroke="currentColor" fill="none" />
        </svg>
    );
}

// a hint's key, a mouse button as a mouse
function HintKey({ hint, platform }: { hint: Hint; platform: Keymap["platform"] }) {
    const { modifiers, name } = splitCombo(hint.key);
    const button = MOUSE_BUTTONS.indexOf(name);
    if (button < 0) return <span className="status-key">{formatCombo(hint.key, platform)}</span>;
    const held = modifiers.map((m) => formatCombo(m, platform)).join(platform === "mac" ? "" : "+");
    return (
        <span className="status-key">
            {held && `${held}${platform === "mac" ? "" : "+"}`}
            <MouseIcon button={button} />
        </span>
    );
}

function StatusBar({ event }: { event: string }) {
    const state = useStateContext().backEndState;
    const message = useStatusMessage();
    const hints = useStatusHints();
    const platform = useKeymapData()?.platform ?? "mac";

    // each write's result stays until the next message
    const write = state?.last_write;
    useEffect(() => {
        const text = lastWriteMessage(write);
        if (text) showStatus(text);
    }, [write?.seq]);

    // Blender connecting and going away, not what it was when the app started
    const connected: boolean | undefined = state?.connected;
    const wasConnected = useRef(connected);
    useEffect(() => {
        if (wasConnected.current !== undefined && connected !== undefined && connected !== wasConnected.current) showStatus(connected ? "Blender connected" : "Blender disconnected");
        wasConnected.current = connected;
    }, [connected]);

    const [version, setVersion] = useState("");
    const [hash, setHash] = useState("");
    useEffect(() => {
        invoke("get_build_info").then((res: any) => {
            const [version, hash] = res;
            setVersion(version);
            setHash(hash);
        });
    }, []);

    // the keys on the left, the last message and the version on the right. each message fades in
    return (
        <div className="status-bar card select-none">
            <div className="panel-header text-[11px] leading-none flex items-center gap-3 px-3 pb-0.5 h-4">
                <div className="flex-1 min-w-0 flex items-center gap-3 py-0.5 overflow-hidden whitespace-nowrap">
                    {hints.map((hint) => (
                        <span key={hint.key} className="flex items-center gap-1">
                            <HintKey hint={hint} platform={platform} />
                            {hint.name}
                        </span>
                    ))}
                </div>
                <div key={message?.seq} className={`flex-none whitespace-nowrap${message ? " status-fade-in" : ""}`}>
                    {message?.text ?? event}
                </div>
                <div className="flex-none whitespace-nowrap">
                    MotionKeys {version} {hash}
                </div>
            </div>
        </div>
    );
}

export default StatusBar;
