import { type CSSProperties, type ReactNode, type Ref, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Glass, SearchField } from "glisse";

// the one menu look (index.css glass menus): the add menus, the tag menu and the dropdowns in nodes. a card of rows on the
// body, so it sits above the floating panels and the canvas (its own stacking context)

// how far from the window's edges a menu is kept
const EDGE = 4;

// where an element sits without the transform glisse's press animation puts on it (usePress scales it about its middle
// and nudges it), so a menu opened from a button that's still springing back doesn't move once it settles
export function layoutRect(element: HTMLElement): DOMRect {
    const rect = element.getBoundingClientRect();
    const [tx = 0, ty = 0] = (element.style.translate || "").split(" ").map((value) => parseFloat(value) || 0);
    const width = element.offsetWidth;
    const height = element.offsetHeight;
    const centerX = rect.left + rect.width / 2 - tx;
    const centerY = rect.top + rect.height / 2 - ty;
    return new DOMRect(centerX - width / 2, centerY - height / 2, width, height);
}

// a menu at a point in the window, moved back inside it when it would hang off the right or bottom edge. `aside` is a
// second card beside it (a category's nodes), a sibling and not inside it: glass inside glass only blurs what's in the
// outer glass
export function MenuCard({ x, y, minWidth, className = "", children, aside }: { x: number; y: number; minWidth?: number; className?: string; children: ReactNode; aside?: ReactNode }) {
    const ref = useRef<HTMLDivElement>(null);
    const [shift, setShift] = useState({ x: 0, y: 0 });
    useLayoutEffect(() => {
        const rect = ref.current?.firstElementChild?.getBoundingClientRect();
        if (!rect) return;
        const right = x + rect.width - (window.innerWidth - EDGE);
        const bottom = y + rect.height - (window.innerHeight - EDGE);
        setShift({ x: right > 0 ? -Math.min(right, x - EDGE) : 0, y: bottom > 0 ? -Math.min(bottom, y - EDGE) : 0 });
    }, [x, y]);

    return createPortal(
        <div ref={ref} style={{ left: x + shift.x, top: y + shift.y }} className="glass-menu-anchor" onMouseDown={(e) => e.stopPropagation()}>
            <MenuPanel className={className} style={minWidth ? { minWidth } : undefined}>
                {children}
            </MenuPanel>
            {aside}
        </div>,
        document.body
    );
}

// one card of rows
export function MenuPanel({ className = "", style, children }: { className?: string; style?: CSSProperties; children: ReactNode }) {
    return (
        <Glass className={`glass-menu ${className}`} style={style}>
            {children}
        </Glass>
    );
}

// the search at a menu's top, typing still reaches it while the menu takes the keys
export function MenuSearch({ value, onChange, placeholder = "", inputRef }: { value: string; onChange: (value: string) => void; placeholder?: string; inputRef?: Ref<HTMLInputElement> }) {
    return <SearchField ref={inputRef} value={value} onValueChange={onChange} placeholder={placeholder} spellCheck={false} />;
}

// one row, `detail` on its right (a category, a submenu's arrow). given `checked`, it keeps room on its left for a check,
// shown on the dropdown's picked row
export function MenuRow({ children, detail, active = false, checked, onClick, onMouseEnter }: { children: ReactNode; detail?: ReactNode; active?: boolean; checked?: boolean; onClick?: () => void; onMouseEnter?: (event: React.MouseEvent<HTMLDivElement>) => void }) {
    return (
        <div className={`glass-menu-row${active ? " active" : ""}${checked ? " picked" : ""}`} onClick={onClick} onMouseEnter={onMouseEnter}>
            {checked !== undefined && <span className="glass-menu-check">{checked ? "✓" : ""}</span>}
            <span className="glass-menu-label truncate">{children}</span>
            {detail != null && <span className="glass-menu-detail">{detail}</span>}
        </div>
    );
}

// a heading over a group of rows (a channel dropdown's object and data groups)
export function MenuHeading({ children }: { children: ReactNode }) {
    return <div className="glass-menu-heading">{children}</div>;
}
