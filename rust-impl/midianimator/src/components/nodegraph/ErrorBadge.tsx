import { useRef, useState } from "react";
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

// what's wrong with the value an input of a node got in the last run
export function useBadInput(nodeId: string, inputId: string | null | undefined): string | undefined {
    const results = useStateContext().backEndState?.executed_results;
    return inputId ? results?.[nodeId]?.[BAD_INPUTS_KEY]?.[inputId] : undefined;
}

// a red warning sign, hovering it shows the message. the popover lives on the body so nodes can't cover it
function ErrorBadge({ message, size }: { message: string; size: number }) {
    const ref = useRef<HTMLDivElement>(null);
    const [anchor, setAnchor] = useState<DOMRect | null>(null);

    return (
        <>
            <div ref={ref} className="error-badge nodrag nopan" style={{ width: size, height: size }} onMouseEnter={() => setAnchor(ref.current?.getBoundingClientRect() ?? null)} onMouseLeave={() => setAnchor(null)}>
                <svg viewBox="0 0 24 24" width="100%" height="100%">
                    <rect x="0" y="0" width="24" height="24" rx="5" className="error-badge-back" />
                    <path d="M12 4.2 L20.6 19.2 Q21 20 20.1 20 H3.9 Q3 20 3.4 19.2 Z" fill="#fff" />
                    <rect x="10.8" y="9" width="2.4" height="6.2" rx="1" className="error-badge-mark" />
                    <circle cx="12" cy="17.4" r="1.3" className="error-badge-mark" />
                </svg>
            </div>
            {anchor &&
                createPortal(
                    <div className="error-popover" style={{ left: anchor.right + 8, top: anchor.top + anchor.height / 2 }}>
                        {message}
                    </div>,
                    document.body
                )}
        </>
    );
}

export default ErrorBadge;
