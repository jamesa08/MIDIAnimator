import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useModal } from "../../utils/keymap";

// one entry of the context menu, `run` is the command it's named after
export type ContextEntry = { id: string; label: string; run: () => void };

// right clicking the graph, like the shift+a menu: `sections` of entries with a line between them, picking one closes
// the menu and runs it. it opens at `position`, flipped left or up when it would go past the window's edge. it's as wide
// as its longest entry
function ContextMenu({ position, sections, onClose }: { position: { x: number; y: number }; sections: ContextEntry[][]; onClose: () => void }) {
    const menuRef = useRef<HTMLDivElement>(null);

    // where it's drawn once its size is known, before it's shown
    const [placed, setPlaced] = useState(position);
    useLayoutEffect(() => {
        const menu = menuRef.current;
        if (!menu) return;
        const { width, height } = menu.getBoundingClientRect();
        const x = position.x + width > window.innerWidth ? position.x - width : position.x;
        const y = position.y + height > window.innerHeight ? position.y - height : position.y;
        setPlaced({ x: Math.max(0, x), y: Math.max(0, y) });
    }, [position.x, position.y]);

    // closes when clicked outside, or when anything scrolls or zooms
    useEffect(() => {
        window.addEventListener("mousedown", onClose);
        window.addEventListener("wheel", onClose, true);
        return () => {
            window.removeEventListener("mousedown", onClose);
            window.removeEventListener("wheel", onClose, true);
        };
    }, [onClose]);

    // takes the keys while it's open
    useModal("context_menu", { cancel: onClose }, true);

    // on the body so it sits above the floating panels, the canvas is its own stacking context
    return createPortal(
        <div ref={menuRef} style={{ position: "fixed", left: placed.x, top: placed.y }} className="context-menu bg-[#2a2a2a] border border-[#444] rounded min-w-[160px] w-max py-1 z-[1000] flex flex-col select-none" onMouseDown={(e) => e.stopPropagation()} onContextMenu={(e) => e.preventDefault()}>
            {sections.map((entries, index) => (
                <div key={index} className={index > 0 ? "border-t border-[#444] mt-1 pt-1" : ""}>
                    {entries.map((entry) => (
                        <div
                            key={entry.id}
                            onClick={() => {
                                onClose();
                                entry.run();
                            }}
                            className="context-menu-row px-2 py-1 cursor-pointer text-white text-[13px] whitespace-nowrap hover:bg-[#4a7ba7]"
                        >
                            {entry.label}
                        </div>
                    ))}
                </div>
            ))}
        </div>,
        document.body
    );
}

export default ContextMenu;
