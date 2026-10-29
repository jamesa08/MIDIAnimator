import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useModal } from "../utils/keymap";
import { MenuHeading, MenuPanel, MenuRow } from "./GlassMenu";

export interface SelectOption {
    value: string;
    label: string;
    // rows with the same group are listed together under its name
    group?: string;
}

// how far from the window's edges the menu is kept
const EDGE = 4;

// a dropdown that opens the glass menu (GlassMenu) instead of the system's, for nodes and the properties panel. it works
// like the system's: the menu opens with the picked row over the field, at the menu's own size whatever the canvas' zoom,
// and while it's open nothing else takes the mouse or keys (the canvas stays put, no other dropdown opens)
function GlassSelect({ value, options, onChange, className = "", disabled = false }: { value: string; options: SelectOption[]; onChange: (value: string) => void; className?: string; disabled?: boolean }) {
    const ref = useRef<HTMLButtonElement>(null);
    // the field on screen when it opened, the menu is placed from it
    const [field, setField] = useState<{ rect: DOMRect; text: DOMRect; width: number } | null>(null);
    // the row the arrow keys are on, the mouse moves it too
    const [highlight, setHighlight] = useState(0);
    const picked = options.findIndex((option) => option.value === value);

    const close = () => setField(null);
    const pick = (index: number) => {
        close();
        const next = options[index];
        if (next && next.value !== value) onChange(next.value);
    };

    // takes every key while it's open: arrows move through the rows, enter picks, escape closes
    useModal(
        "dropdown",
        {
            up: () => setHighlight((index) => Math.max(0, index - 1)),
            down: () => setHighlight((index) => Math.min(options.length - 1, index + 1)),
            confirm: () => pick(highlight),
            cancel: close,
        },
        !!field
    );

    const open = () => {
        const button = ref.current;
        const text = button?.firstElementChild;
        if (!button || !text) return;
        setHighlight(Math.max(0, picked));
        setField({ rect: button.getBoundingClientRect(), text: text.getBoundingClientRect(), width: button.offsetWidth });
    };

    const label = options[picked]?.label ?? "";

    return (
        <>
            <button ref={ref} type="button" className={`glass-select ${className}`} disabled={disabled} aria-expanded={!!field} onMouseDown={(e) => e.stopPropagation()} onClick={open}>
                <span className="truncate">{label}</span>
                <svg viewBox="0 0 8 10" width="8" height="10" fill="none" stroke="currentColor" aria-hidden>
                    <path d="M1 3.5l3-3 3 3M1 6.5l3 3 3-3" />
                </svg>
            </button>
            {field && <DropdownMenu field={field} options={options} picked={picked} highlight={highlight} onHighlight={setHighlight} onPick={pick} onClose={close} />}
        </>
    );
}

// the open menu, on the body above everything. a see-through layer over the whole window under it takes the clicks and
// scrolls meant for anything else: a click there only closes the menu
function DropdownMenu({ field, options, picked, highlight, onHighlight, onPick, onClose }: { field: { rect: DOMRect; text: DOMRect; width: number }; options: SelectOption[]; picked: number; highlight: number; onHighlight: (index: number) => void; onPick: (index: number) => void; onClose: () => void }) {
    const anchorRef = useRef<HTMLDivElement>(null);
    const [place, setPlace] = useState<{ x: number; y: number } | null>(null);

    // the picked row's text sits over the field's text, like the system's dropdown, then the menu is kept inside the window.
    // measured before it's first drawn, so it never shows in the wrong place
    useLayoutEffect(() => {
        const card = anchorRef.current?.firstElementChild as HTMLElement | null;
        if (!card) return;
        const row = card.querySelector<HTMLElement>(".glass-menu-row.picked") ?? card.querySelector<HTMLElement>(".glass-menu-row");
        row?.scrollIntoView({ block: "nearest" });
        const box = card.getBoundingClientRect();
        const rowBox = row?.getBoundingClientRect() ?? box;
        const rowText = row?.querySelector(".glass-menu-label")?.getBoundingClientRect() ?? rowBox;
        const x = field.text.left - (rowText.left - box.left);
        const y = field.rect.top + field.rect.height / 2 - (rowBox.top - box.top + rowBox.height / 2);
        setPlace({
            x: Math.max(EDGE, Math.min(x, window.innerWidth - box.width - EDGE)),
            y: Math.max(EDGE, Math.min(y, window.innerHeight - box.height - EDGE)),
        });
    }, [field]);

    return createPortal(
        <>
            <div
                className="glass-select-catcher"
                onMouseDown={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                }}
                onClick={onClose}
                onContextMenu={(e) => {
                    e.preventDefault();
                    onClose();
                }}
            />
            <div ref={anchorRef} className="glass-menu-anchor" style={{ left: place?.x ?? field.rect.left, top: place?.y ?? field.rect.top, visibility: place ? undefined : "hidden" }} onMouseDown={(e) => e.stopPropagation()}>
                <MenuPanel className="glass-select-menu" style={{ minWidth: field.width }}>
                    <div className="glass-menu-scroll">
                        {options.map((option, index) => (
                            <div key={option.value}>
                                {option.group != null && option.group !== options[index - 1]?.group && <MenuHeading>{option.group}</MenuHeading>}
                                <MenuRow active={index === highlight} checked={index === picked} onMouseEnter={() => onHighlight(index)} onClick={() => onPick(index)}>
                                    {option.label}
                                </MenuRow>
                            </div>
                        ))}
                    </div>
                </MenuPanel>
            </div>
        </>,
        document.body
    );
}

export default GlassSelect;
