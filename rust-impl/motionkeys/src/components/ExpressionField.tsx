import { CSSProperties, useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ExprError } from "../utils/mapper";

// a single line expression (src-tauri/src/utils/expr.rs), checked by the backend while it's typed: invalid text gets a red
// border and what's wrong underlined. it's set when it's left or on enter, escape puts it back. `vars` are the names the
// expression can use, each place it's used gives its own
function ExpressionField({ value, vars, onCommit, readOnly, className, style }: { value: string; vars: string[]; onCommit: (value: string) => void; readOnly?: boolean; className?: string; style?: CSSProperties }) {
    const [text, setText] = useState(value);
    const [error, setError] = useState<ExprError | null>(null);
    const inputRef = useRef<HTMLInputElement>(null);
    const mirrorRef = useRef<HTMLDivElement>(null);
    useEffect(() => setText(value), [value]);

    // check what's typed, the newest answer wins
    const varsKey = vars.join(",");
    useEffect(() => {
        // empty is fine, it changes nothing
        if (text.trim() === "") {
            setError(null);
            return;
        }
        let live = true;
        const timer = setTimeout(() => {
            invoke<ExprError | null>("check_expression", { text, vars })
                .then((e) => live && setError(e ?? null))
                .catch(() => live && setError(null));
        }, 80);
        return () => {
            live = false;
            clearTimeout(timer);
        };
    }, [text, varsKey]);

    // the underline is drawn on a copy of the text over the input, laid out like it and scrolled with it. it's a border, webkit
    // doesn't paint the text decorations of transparent text
    useLayoutEffect(() => {
        const input = inputRef.current;
        const mirror = mirrorRef.current;
        if (!input || !mirror) return;
        const s = getComputedStyle(input);
        Object.assign(mirror.style, {
            font: s.font,
            letterSpacing: s.letterSpacing,
            paddingLeft: s.paddingLeft,
            paddingRight: s.paddingRight,
            borderLeft: `${s.borderLeftWidth} solid transparent`,
            borderRight: `${s.borderRightWidth} solid transparent`,
            lineHeight: `${input.clientHeight}px`,
            top: `${input.offsetTop + parseFloat(s.borderTopWidth)}px`,
            left: `${input.offsetLeft}px`,
            width: `${input.offsetWidth}px`,
            height: `${input.clientHeight}px`,
        });
        mirror.scrollLeft = input.scrollLeft;
    });
    const sync = () => {
        if (mirrorRef.current && inputRef.current) mirrorRef.current.scrollLeft = inputRef.current.scrollLeft;
    };

    const commit = () => {
        if (text !== value) onCommit(text);
    };

    // an error at the end (something missing) underlines a space after the text
    const chars = Array.from(text);
    const start = error ? Math.min(error.start, chars.length) : 0;
    const end = error ? Math.max(Math.min(error.end, chars.length), start) : 0;
    const marked = error ? chars.slice(start, end).join("") || " " : "";

    return (
        <>
            <input
                ref={inputRef}
                type="text"
                spellCheck={false}
                autoComplete="off"
                className={`${className ?? ""} nodrag font-mono`}
                style={{ ...style, ...(error ? { borderColor: "#dc2626", outlineColor: "#dc2626" } : {}) }}
                value={text}
                readOnly={readOnly}
                onChange={(e) => setText(e.target.value)}
                onScroll={sync}
                onSelect={sync}
                onKeyUp={sync}
                onBlur={commit}
                onKeyDown={(e) => {
                    if (e.key === "Enter") e.currentTarget.blur();
                    if (e.key === "Escape") {
                        setText(value);
                        requestAnimationFrame(() => (e.target as HTMLInputElement).blur());
                    }
                }}
            />
            {error && (
                <div ref={mirrorRef} aria-hidden style={{ position: "absolute", overflow: "hidden", whiteSpace: "pre", color: "transparent", pointerEvents: "none", boxSizing: "border-box" }}>
                    {chars.slice(0, start).join("")}
                    <span className="expression-error-mark" style={{ borderBottom: "2px solid #dc2626" }}>
                        {marked}
                    </span>
                    {chars.slice(end).join("")}
                </div>
            )}
        </>
    );
}

export default ExpressionField;
