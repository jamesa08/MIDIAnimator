import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useModal } from "../../utils/keymap";

// double clicking a socket, like the shift+a menu: searches `names` (an input gets the outputs' tags, an output the broken
// tags on inputs), a name nothing has yet comes after them. enter picks the first
function TagMenu({ current, names, position, onClose, onSubmit }: { current: string; names: string[]; position: { x: number; y: number }; onClose: () => void; onSubmit: (name: string) => void }) {
    const [search, setSearch] = useState("");
    const searchInputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        searchInputRef.current?.focus();
    }, []);

    // closes when clicked outside
    useEffect(() => {
        window.addEventListener("mousedown", onClose);
        return () => window.removeEventListener("mousedown", onClose);
    }, [onClose]);

    const typed = search.trim();
    const matches = names.filter((name) => name !== current && name.toLowerCase().includes(typed.toLowerCase()));
    const rows = typed && !names.includes(typed) ? [...matches, typed] : matches;

    // takes the keys while it's open, typing still goes to the search
    useModal(
        "tag_menu",
        {
            confirm: () => {
                if (rows.length > 0) onSubmit(rows[0]);
            },
            cancel: onClose,
        },
        true,
        { passthrough: true }
    );

    // on the body so it sits above the floating panels, the canvas is its own stacking context
    return createPortal(
        <div style={{ position: "fixed", left: position.x, top: position.y }} className="tag-menu bg-[#2a2a2a] border border-[#444] rounded w-[250px] max-h-[400px] z-[1000] flex flex-col" onMouseDown={(e) => e.stopPropagation()}>
            <input ref={searchInputRef} type="text" value={search} onChange={(e) => setSearch(e.target.value)} placeholder={current ? "Rename tag" : "Add tag"} spellCheck={false} className="px-2 py-1 bg-[#1a1a1a] border-0 border-b border-[#444] text-white outline-none text-[13px]" />
            <div style={{ overflowY: "auto", maxHeight: "350px" }}>
                {rows.map((name) => (
                    <div key={name} onClick={() => onSubmit(name)} className="tag-menu-row px-2 py-1 cursor-pointer text-white text-[13px] truncate">
                        {name}
                    </div>
                ))}
            </div>
        </div>,
        document.body
    );
}

export default TagMenu;
