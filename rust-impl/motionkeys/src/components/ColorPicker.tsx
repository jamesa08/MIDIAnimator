import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import Wheel from "@uiw/react-color-wheel";
import ShadeSlider from "@uiw/react-color-shade-slider";
import { HsvaColor, hexToHsva, hsvaToHex, hsvaToRgba, rgbaToHsva } from "@uiw/color-convert";
import { useModal } from "../utils/keymap";
import { ValueSlider } from "./ValueSlider";

// a color picker like Blender's: a hue and saturation wheel with the value beside it (@uiw/react-color), the color's
// three channels below as sliders (RGB or HSV), and its hex code. `value` and what `onChange` gets are hex codes (#rrggbb)

// the wheel goes like blender's: red at the bottom, then yellow, green and cyan going clockwise. it draws its hues
// turned by 90 - angle but puts its handle at angle + hue, so at angle 270 the handle is half a turn from the hue under
// it. it's given every hue half a turn on (HUE_TURN), and what it picks comes back half a turn the other way
const WHEEL = { angle: 270, direction: "clockwise" as const };
const HUE_TURN = 180;
const turnHue = (h: number, by: number) => (((h + by) % 360) + 360) % 360;
// the value slider's width and the gap between it and the wheel
const SLIDER_SIZE = 16;
const SLIDER_MARGIN = 12;
// the channels the sliders show, 0 to 1 like blender's, and the color with one of them changed
type Channel = { label: string; get: (hsva: HsvaColor) => number; set: (hsva: HsvaColor, value: number) => HsvaColor };
const rgb = (key: "r" | "g" | "b", label: string): Channel => ({ label, get: (c) => hsvaToRgba(c)[key] / 255, set: (c, v) => rgbaToHsva({ ...hsvaToRgba(c), [key]: Math.round(v * 255) }) });
const CHANNELS: Record<"rgb" | "hsv", Channel[]> = {
    rgb: [rgb("r", "Red"), rgb("g", "Green"), rgb("b", "Blue")],
    hsv: [
        { label: "Hue", get: (c) => c.h / 360, set: (c, v) => ({ ...c, h: v * 360 }) },
        { label: "Saturation", get: (c) => c.s / 100, set: (c, v) => ({ ...c, s: v * 100 }) },
        { label: "Value", get: (c) => c.v / 100, set: (c, v) => ({ ...c, v: v * 100 }) },
    ],
};

// a hex code from what was typed: #rgb or #rrggbb, with or without the #. null when it isn't one
function parseHex(text: string): string | null {
    const hex = text.trim().replace(/^#/, "");
    if (/^[0-9a-f]{3}$/i.test(hex)) return `#${[...hex].map((c) => c + c).join("")}`.toLowerCase();
    if (/^[0-9a-f]{6}$/i.test(hex)) return `#${hex}`.toLowerCase();
    return null;
}

const toHex = (hsva: HsvaColor) => hsvaToHex(hsva).toLowerCase();

// the picker itself, `size` is the wheel's width
export function ColorPicker({ value, onChange, size = 160 }: { value: string; onChange: (hex: string) => void; size?: number }) {
    const [mode, setMode] = useState<"rgb" | "hsv">("hsv");
    // the color as it's being picked, it keeps the hue and saturation of a black or a gray
    const [hsva, setHsva] = useState<HsvaColor>(() => hexToHsva(parseHex(value) ?? "#000000"));

    // a new value from outside shows without being sent back
    useEffect(() => {
        const hex = parseHex(value);
        if (hex && hex !== toHex(hsva)) setHsva(hexToHsva(hex));
    }, [value]);

    const pick = (next: HsvaColor) => {
        setHsva(next);
        onChange(toHex(next));
    };

    return (
        <div className="color-picker" style={{ width: size + SLIDER_MARGIN + SLIDER_SIZE }}>
            <div className="color-picker-wheel" style={{ gap: SLIDER_MARGIN }}>
                {/* the wheel stays bright like blender's, the value is beside it. its handle shows the color picked */}
                <Wheel
                    color={{ ...hsva, h: turnHue(hsva.h, HUE_TURN), v: 100 }}
                    width={size}
                    height={size}
                    {...WHEEL}
                    pointer={({ style }) => (
                        <div style={style}>
                            <div className="color-picker-handle" style={{ background: toHex({ ...hsva, v: 100 }) }} />
                        </div>
                    )}
                    onChange={(color) => pick({ ...hsva, h: turnHue(color.hsva.h, -HUE_TURN), s: color.hsva.s })}
                />
                {/* bright at the top, its handle is drawn upside down unless it's reversed */}
                <ShadeSlider hsva={hsva} direction="vertical" reverse width={SLIDER_SIZE} height={size} onChange={({ v }) => pick({ ...hsva, v: Math.min(100, Math.max(0, v)) })} />
            </div>
            <div className="color-picker-modes">
                {(["rgb", "hsv"] as const).map((m) => (
                    <button key={m} className={m === mode ? "active" : ""} onClick={() => setMode(m)}>
                        {m.toUpperCase()}
                    </button>
                ))}
            </div>
            <div className="color-picker-sliders">
                {CHANNELS[mode].map((channel) => (
                    <ValueSlider key={channel.label} label={channel.label} value={channel.get(hsva)} onChange={(v) => pick(channel.set(hsva, v))} />
                ))}
            </div>
            <div className="color-picker-hex">
                <span>Hex</span>
                <HexField
                    value={toHex(hsva).toUpperCase()}
                    onCommit={(text) => {
                        const hex = parseHex(text);
                        if (hex && hex !== toHex(hsva)) pick(hexToHsva(hex));
                    }}
                />
            </div>
        </div>
    );
}

// the hex code, set when it's left or on enter, anything that isn't a color is put back
function HexField({ value, onCommit, readOnly }: { value: string; onCommit: (text: string) => void; readOnly?: boolean }) {
    const [text, setText] = useState(value);
    useEffect(() => setText(value), [value]);
    return (
        <input
            value={text}
            readOnly={readOnly}
            spellCheck={false}
            onChange={(e) => setText(e.target.value)}
            onBlur={() => {
                onCommit(text);
                setText(value);
            }}
            onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
            }}
        />
    );
}

// a color as a square swatch and its hex code: the swatch opens the picker below it (above when there's no room), the
// hex code can be typed in. `onChange` gets every change, the picker's while it's open, `onClose` the color it was
// closed on
export function ColorField({ value, onChange, onOpen, onClose, readOnly }: { value: string; onChange: (hex: string) => void; onOpen?: () => void; onClose?: (hex: string) => void; readOnly?: boolean }) {
    const buttonRef = useRef<HTMLButtonElement>(null);
    const [open, setOpen] = useState<{ x: number; y: number } | null>(null);
    // the last color picked while it's open
    const pickedRef = useRef(value);
    const change = (hex: string) => {
        pickedRef.current = hex;
        onChange(hex);
    };
    const close = () => {
        setOpen(null);
        onClose?.(pickedRef.current);
    };

    return (
        <div className="color-field">
            <button
                ref={buttonRef}
                className="color-swatch"
                disabled={readOnly}
                style={{ background: value }}
                onClick={() => {
                    const rect = buttonRef.current!.getBoundingClientRect();
                    pickedRef.current = value;
                    setOpen({ x: rect.left, y: rect.bottom });
                    onOpen?.();
                }}
            />
            <HexField
                value={value.toUpperCase()}
                readOnly={readOnly}
                onCommit={(text) => {
                    const hex = parseHex(text);
                    if (hex && hex !== value.toLowerCase()) onChange(hex);
                }}
            />
            {open && <ColorPopover position={open} anchorTop={buttonRef.current?.getBoundingClientRect().top ?? open.y} value={value} onChange={change} onClose={close} />}
        </div>
    );
}

// the picker floating over everything, closed by a click outside it or escape. it keeps the color it's showing, an
// edit on its way back doesn't move it, and sends changes at most once a frame
function ColorPopover({ position, anchorTop, value, onChange, onClose }: { position: { x: number; y: number }; anchorTop: number; value: string; onChange: (hex: string) => void; onClose: () => void }) {
    const popoverRef = useRef<HTMLDivElement>(null);
    const [shown, setShown] = useState(value);
    const onChangeRef = useRef(onChange);
    onChangeRef.current = onChange;
    const pendingRef = useRef<string | null>(null);
    const flush = () => {
        if (pendingRef.current !== null) onChangeRef.current(pendingRef.current);
        pendingRef.current = null;
    };
    const change = (hex: string) => {
        setShown(hex);
        if (pendingRef.current === null) requestAnimationFrame(flush);
        pendingRef.current = hex;
    };
    // kept inside the window, flipped above the swatch when it would go past the bottom
    const [placed, setPlaced] = useState(position);
    useLayoutEffect(() => {
        const { width, height } = popoverRef.current!.getBoundingClientRect();
        const y = position.y + height > window.innerHeight ? anchorTop - height : position.y;
        setPlaced({ x: Math.max(0, Math.min(position.x, window.innerWidth - width)), y: Math.max(0, y) });
    }, [position.x, position.y, anchorTop]);

    // a change still waiting goes out before it closes
    const onCloseRef = useRef(onClose);
    onCloseRef.current = () => {
        flush();
        onClose();
    };
    useEffect(() => {
        const outside = () => onCloseRef.current();
        window.addEventListener("mousedown", outside);
        return () => window.removeEventListener("mousedown", outside);
    }, []);
    // takes the keys while it's open, typing still goes to its fields
    useModal("color_picker", { cancel: () => onCloseRef.current() }, true, { passthrough: true });

    return createPortal(
        <div ref={popoverRef} className="color-popover" style={{ left: placed.x, top: placed.y }} onMouseDown={(e) => e.stopPropagation()}>
            <ColorPicker value={shown} onChange={change} />
        </div>,
        document.body
    );
}

export default ColorPicker;
