import { useState } from "react";

// a value from 0 to 1 as a bar filled that far, its name on the left and the number on the right. dragging across it
// sets it, a double click types it in. `onStart` and `onEnd` are called when a drag starts and ends
export function ValueSlider({ label, value, onChange, onStart, onEnd, className }: { label?: string; value: number; onChange: (value: number) => void; onStart?: () => void; onEnd?: () => void; className?: string }) {
    const [typing, setTyping] = useState(false);
    const classes = `value-slider${className ? ` ${className}` : ""}`;
    const at = (event: React.PointerEvent<HTMLDivElement>) => {
        const rect = event.currentTarget.getBoundingClientRect();
        onChange(Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)));
    };
    if (typing) {
        const commit = (text: string) => {
            setTyping(false);
            if (text.trim() !== "" && !isNaN(Number(text))) onChange(Math.min(1, Math.max(0, Number(text))));
        };
        return (
            <input
                className={classes}
                autoFocus
                defaultValue={value.toFixed(3)}
                onFocus={(e) => e.target.select()}
                onBlur={(e) => commit(e.target.value)}
                onKeyDown={(e) => {
                    if (e.key === "Enter") e.currentTarget.blur();
                }}
            />
        );
    }
    return (
        <div
            className={classes}
            onPointerDown={(e) => {
                e.currentTarget.setPointerCapture(e.pointerId);
                onStart?.();
                at(e);
            }}
            onPointerMove={(e) => e.currentTarget.hasPointerCapture(e.pointerId) && at(e)}
            onLostPointerCapture={() => onEnd?.()}
            onDoubleClick={() => setTyping(true)}
        >
            <div className="value-slider-fill" style={{ width: `${value * 100}%` }} />
            {label && <span>{label}</span>}
            <span>{value.toFixed(3)}</span>
        </div>
    );
}
