import { ReactNode, useEffect, useRef, useState } from "react";
import { CLICK_DISTANCE } from "./nodegraph/canvasInput";

// text in a box, its name on the left and the text on the right, like NumberBox and Blender's fields. every text typed in
// on a node is one, beside its socket. pressing on it drags its node like the rest of the node, a click (a press that
// didn't move) types in it: only then does the text take the pointer (nodrag, nopan). it's set when it's left or on
// enter (only a change is sent), escape puts it back. `children` takes the place of the text, e.g. Animation Generator's
// mappers put their expression field there
export function TextBox({ label, value, onCommit, readOnly, className, children }: { label?: string; value?: string; onCommit?: (text: string) => void; readOnly?: boolean; className?: string; children?: ReactNode }) {
    const [text, setText] = useState(value ?? "");
    useEffect(() => setText(value ?? ""), [value]);
    // typing in it, its input has focus
    const [editing, setEditing] = useState(false);
    const boxRef = useRef<HTMLDivElement>(null);
    const press = useRef<{ x: number; y: number } | null>(null);

    return (
        <div
            ref={boxRef}
            className={`text-box border border-gray-400 rounded${editing ? " editing nodrag nopan" : ""}${className ? ` ${className}` : ""}`}
            onPointerDown={(e) => (press.current = { x: e.clientX, y: e.clientY })}
            onClick={(e) => {
                const from = press.current;
                press.current = null;
                if (readOnly || editing || (from && Math.hypot(e.clientX - from.x, e.clientY - from.y) > CLICK_DISTANCE)) return;
                // the caret goes at the end of the text
                const input = boxRef.current?.querySelector("input");
                input?.focus();
                input?.setSelectionRange(input.value.length, input.value.length);
            }}
            onFocus={() => setEditing(true)}
            onBlur={() => setEditing(false)}
        >
            {label && <span className="text-box-label">{label}</span>}
            {children ?? (
                <input
                    type="text"
                    className="text-box-input"
                    spellCheck={false}
                    autoComplete="off"
                    value={text}
                    readOnly={readOnly}
                    onChange={(e) => setText(e.target.value)}
                    onBlur={() => text !== (value ?? "") && onCommit?.(text)}
                    onKeyDown={(e) => {
                        if (e.key === "Enter") e.currentTarget.blur();
                        if (e.key === "Escape") {
                            setText(value ?? "");
                            requestAnimationFrame(() => (e.target as HTMLInputElement).blur());
                        }
                    }}
                />
            )}
        </div>
    );
}
