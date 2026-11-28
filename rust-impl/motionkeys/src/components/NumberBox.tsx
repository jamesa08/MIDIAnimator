import { useEffect, useRef, useState } from "react";
import { CLICK_DISTANCE } from "./nodegraph/canvasInput";

// a number in a box, its name on the left and the number on the right, like Blender's fields. every number on a node is
// one: BaseNode draws one beside each number socket. a click types it in, enter or clicking away sets it and escape
// leaves it. emptied, `onClear` is called (back to the default), without one it's left as it was. `onDraft` hears each
// number as it's typed, null while it isn't one or once typing is left. an `integer` box shows no decimals and takes
// only whole numbers.
// a `slider` is a value from 0 to 1 filled that far: dragging across sets it and a double click types it in. `onStart`
// and `onEnd` are called when a drag starts and ends. a `readOnly` box only shows its number (previews).
// pressing on a plain box drags its node like the rest of the node, only a click (a press that didn't move) types in it.
// a slider's drags and the box being typed in are its own (nodrag, nopan)
export function NumberBox({
    label,
    value,
    onChange,
    onClear,
    onDraft,
    slider,
    integer,
    onStart,
    onEnd,
    readOnly,
    className,
    style,
}: {
    label?: string;
    value: number | undefined;
    onChange?: (value: number) => void;
    onClear?: () => void;
    onDraft?: (value: number | null) => void;
    slider?: boolean;
    integer?: boolean;
    onStart?: () => void;
    onEnd?: () => void;
    readOnly?: boolean;
    className?: string;
    style?: React.CSSProperties;
}) {
    // the number as it showed when typing started, null while it isn't typed in
    const [typing, setTyping] = useState<string | null>(null);
    // escape leaves the number as it was
    const cancelRef = useRef(false);
    // a press on the box: where it went down, and whether it moved far enough to be a drag
    const pressRef = useRef<{ x: number; y: number; dragging: boolean } | null>(null);
    const classes = `number-box${slider ? " number-box-slider nodrag nopan" : ""}${className ? ` ${className}` : ""}`;
    const shown = typeof value === "number" ? value : undefined;
    const format = (number: number) => (integer ? String(number) : number.toFixed(3));

    // a drag cut off by the box going away (its node collapsed, its input connected) still ends
    const onEndRef = useRef(onEnd);
    onEndRef.current = onEnd;
    useEffect(() => {
        return () => {
            if (pressRef.current?.dragging) onEndRef.current?.();
            pressRef.current = null;
        };
    }, []);

    const startTyping = () => {
        if (readOnly) return;
        cancelRef.current = false;
        setTyping(shown === undefined ? "" : slider ? shown.toFixed(3) : String(shown));
    };
    const parse = (text: string) => (text.trim() === "" || isNaN(Number(text)) || (integer && !Number.isInteger(Number(text))) ? null : Number(text));
    const clamp = (number: number) => (slider ? Math.min(1, Math.max(0, number)) : number);

    if (typing !== null) {
        const commit = (text: string) => {
            setTyping(null);
            onDraft?.(null);
            // escape, or left as it showed: nothing was typed
            const cancelled = cancelRef.current;
            cancelRef.current = false;
            if (cancelled || text === typing) return;
            const number = parse(text);
            if (number !== null) onChange?.(clamp(number));
            else if (text.trim() === "") onClear?.();
        };
        return (
            <input
                className={`${classes} number-box-typing nodrag nopan`}
                style={style}
                autoFocus
                defaultValue={typing}
                onFocus={(e) => e.target.select()}
                onChange={(e) => onDraft?.(parse(e.target.value))}
                onBlur={(e) => commit(e.target.value)}
                onKeyDown={(e) => {
                    if (e.key === "Enter") e.currentTarget.blur();
                    if (e.key === "Escape") {
                        cancelRef.current = true;
                        e.currentTarget.blur();
                    }
                }}
            />
        );
    }

    // a slider follows the pointer across it
    const at = (event: React.PointerEvent<HTMLDivElement>) => {
        const rect = event.currentTarget.getBoundingClientRect();
        onChange?.(Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)));
    };
    return (
        <div
            className={classes}
            style={style}
            onPointerDown={(e) => {
                if (readOnly) return;
                if (slider) e.currentTarget.setPointerCapture(e.pointerId);
                pressRef.current = { x: e.clientX, y: e.clientY, dragging: false };
            }}
            onPointerMove={(e) => {
                const press = pressRef.current;
                if (!slider || !press || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
                // a drag once it's moved further than a click can, from then on it follows the pointer
                if (!press.dragging) {
                    if (Math.abs(e.clientX - press.x) <= CLICK_DISTANCE) return;
                    press.dragging = true;
                    onStart?.();
                }
                at(e);
            }}
            onClick={(e) => {
                // a plain box types in on a click, not after its node was dragged by it
                const press = pressRef.current;
                if (slider || !press) return;
                pressRef.current = null;
                if (Math.hypot(e.clientX - press.x, e.clientY - press.y) <= CLICK_DISTANCE) startTyping();
            }}
            onLostPointerCapture={() => {
                const press = pressRef.current;
                pressRef.current = null;
                if (press?.dragging) onEnd?.();
            }}
            onDoubleClick={() => slider && startTyping()}
        >
            {slider && <div className="number-box-fill" style={{ width: `${(shown ?? 0) * 100}%` }} />}
            {label && <span className="number-box-label">{label}</span>}
            <span className="number-box-value">{shown === undefined ? "" : format(shown)}</span>
        </div>
    );
}
