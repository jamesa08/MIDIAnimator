import { useEffect, useRef, useState } from "react";
import { CLICK_DISTANCE } from "./nodegraph/NodeGraphCanvas";

// a value from 0 to 1 as a bar filled that far, its name on the left and the number on the right. dragging across it
// sets it, a double click types it in. `onStart` and `onEnd` are called when a drag starts and ends
export function ValueSlider({ label, value, onChange, onStart, onEnd, className }: { label?: string; value: number; onChange: (value: number) => void; onStart?: () => void; onEnd?: () => void; className?: string }) {
    // the number as it showed when typing started, null while it isn't typed in
    const [typing, setTyping] = useState<string | null>(null);
    // escape leaves the number as it was
    const cancelRef = useRef(false);
    // a press on the bar: where it went down, and whether it moved far enough to be a drag. a click alone changes nothing
    const pressRef = useRef<{ x: number; dragging: boolean } | null>(null);
    const classes = `value-slider${className ? ` ${className}` : ""}`;

    // a drag cut off by the slider going away (its node collapsed, its input connected) still ends
    const onEndRef = useRef(onEnd);
    onEndRef.current = onEnd;
    useEffect(() => {
        return () => {
            if (pressRef.current?.dragging) onEndRef.current?.();
            pressRef.current = null;
        };
    }, []);

    const at = (event: React.PointerEvent<HTMLDivElement>) => {
        const rect = event.currentTarget.getBoundingClientRect();
        onChange(Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)));
    };
    if (typing !== null) {
        const commit = (text: string) => {
            setTyping(null);
            // escape, or left as it showed: nothing was typed
            const cancelled = cancelRef.current;
            cancelRef.current = false;
            if (cancelled || text === typing) return;
            if (text.trim() !== "" && !isNaN(Number(text))) onChange(Math.min(1, Math.max(0, Number(text))));
        };
        return (
            <input
                className={classes}
                autoFocus
                defaultValue={typing}
                onFocus={(e) => e.target.select()}
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
    return (
        <div
            className={classes}
            onPointerDown={(e) => {
                e.currentTarget.setPointerCapture(e.pointerId);
                pressRef.current = { x: e.clientX, dragging: false };
            }}
            onPointerMove={(e) => {
                const press = pressRef.current;
                if (!press || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
                // a drag once it's moved further than a click can, from then on it follows the pointer
                if (!press.dragging) {
                    if (Math.abs(e.clientX - press.x) <= CLICK_DISTANCE) return;
                    press.dragging = true;
                    onStart?.();
                }
                at(e);
            }}
            onLostPointerCapture={() => {
                const press = pressRef.current;
                pressRef.current = null;
                if (press?.dragging) onEnd?.();
            }}
            onDoubleClick={() => {
                cancelRef.current = false;
                setTyping(value.toFixed(3));
            }}
        >
            <div className="value-slider-fill" style={{ width: `${value * 100}%` }} />
            {label && <span>{label}</span>}
            <span>{value.toFixed(3)}</span>
        </div>
    );
}
