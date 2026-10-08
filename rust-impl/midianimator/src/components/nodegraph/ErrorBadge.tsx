import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useNodeId } from "@xyflow/react";
import { useStateContext } from "../../contexts/StateContext";

// keys a failed node's executed results hold its error under, see src-tauri/src/graph/executors/io.rs
export const ERROR_KEY = "motionkeys_error";
export const BAD_INPUTS_KEY = "motionkeys_bad_inputs";

// the error of the node this is rendered in, from the last run
export function useNodeError(): string | undefined {
    const id = useNodeId();
    const results = useStateContext().backEndState?.executed_results;
    return id ? results?.[id]?.[ERROR_KEY] : undefined;
}

// the last write of the scene writers the node this is rendered in is or contains (a group node, at any depth):
// the first error, or written when they all went through. nothing when none of them has written yet.
// realtime runs skip the writers, they keep what their last write gave
export function useWriteStatus(): { error?: string; written: boolean } {
    const id = useNodeId();
    const results = useStateContext().backEndState?.executed_results;
    if (!id || !results) return { written: false };
    const writers = Object.keys(results).filter((path) => (path === id || path.startsWith(`${id}/`)) && /^scene_writer-/.test(path.slice(path.lastIndexOf("/") + 1)));
    const error = writers.map((path) => results[path]?.[ERROR_KEY]).find((message) => message != null);
    return { error, written: writers.length > 0 && error == null };
}

// what's wrong with the value an input of a node got in the last run
export function useBadInput(nodeId: string, inputId: string | null | undefined): string | undefined {
    const results = useStateContext().backEndState?.executed_results;
    return inputId ? results?.[nodeId]?.[BAD_INPUTS_KEY]?.[inputId] : undefined;
}

// how long the popover stays after the mouse leaves the sign, so it can move across the gap into it
const CLOSE_DELAY = 200;

// a red warning sign, hovering it shows the message. the popover lives on the body so nodes can't cover it,
// it stays open while hovered so the message can be selected or copied
function ErrorBadge({ message, size }: { message: string; size: number }) {
    const ref = useRef<HTMLDivElement>(null);
    const [anchor, setAnchor] = useState<DOMRect | null>(null);
    const [copied, setCopied] = useState(false);
    const closeTimer = useRef<number | undefined>(undefined);

    const open = () => {
        window.clearTimeout(closeTimer.current);
        setAnchor(ref.current?.getBoundingClientRect() ?? null);
    };
    const close = () => {
        window.clearTimeout(closeTimer.current);
        closeTimer.current = window.setTimeout(() => {
            setAnchor(null);
            setCopied(false);
        }, CLOSE_DELAY);
    };
    const keepOpen = () => window.clearTimeout(closeTimer.current);
    useEffect(() => () => window.clearTimeout(closeTimer.current), []);

    const copy = () => {
        navigator.clipboard
            .writeText(message)
            .then(() => setCopied(true))
            .catch((e) => console.error(`Error copying error message: ${e}`));
    };

    return (
        <>
            <div ref={ref} className="error-badge nodrag nopan" style={{ width: size, height: size }} onMouseEnter={open} onMouseLeave={close}>
                <svg viewBox="0 0 24 24" width="100%" height="100%">
                    <rect x="0" y="0" width="24" height="24" rx="5" className="error-badge-back" />
                    <path d="M12 4.2 L20.6 19.2 Q21 20 20.1 20 H3.9 Q3 20 3.4 19.2 Z" fill="#fff" />
                    <rect x="10.8" y="9" width="2.4" height="6.2" rx="1" className="error-badge-mark" />
                    <circle cx="12" cy="17.4" r="1.3" className="error-badge-mark" />
                </svg>
            </div>
            {anchor &&
                createPortal(
                    <div className="error-popover" style={{ left: anchor.right + 8, top: anchor.top + anchor.height / 2 }} onMouseEnter={keepOpen} onMouseLeave={close}>
                        <div className="error-popover-message">{message}</div>
                        <button className="error-popover-copy" onClick={copy}>
                            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                {copied ? (
                                    <path d="M5 12.5 L10 17.5 L19 7" />
                                ) : (
                                    <>
                                        <rect x="9" y="9" width="11" height="11" rx="2" />
                                        <path d="M5 15 H4.5 A1.5 1.5 0 0 1 3 13.5 V4.5 A1.5 1.5 0 0 1 4.5 3 H13.5 A1.5 1.5 0 0 1 15 4.5 V5" />
                                    </>
                                )}
                            </svg>
                        </button>
                    </div>,
                    document.body
                )}
        </>
    );
}

// a green check, the last write to Blender went through
export function SuccessBadge({ size }: { size: number }) {
    return (
        <div className="success-badge nodrag nopan" style={{ width: size, height: size }}>
            <svg viewBox="0 0 24 24" width="100%" height="100%">
                <rect x="0" y="0" width="24" height="24" rx="5" className="success-badge-back" />
                <path d="M6.5 12.5 L10.5 16.5 L17.5 8" fill="none" stroke="#fff" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
        </div>
    );
}

export default ErrorBadge;
