import { useEffect, useRef, useState } from "react";
import { useModal } from "../../utils/keymap";
import { MenuCard, MenuRow, MenuSearch } from "../GlassMenu";

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

    return (
        <MenuCard x={position.x} y={position.y} className="w-[250px] tag-menu">
            <MenuSearch inputRef={searchInputRef} value={search} onChange={setSearch} placeholder={current ? "Rename tag" : "Add tag"} />
            <div className="glass-menu-scroll">
                {rows.map((name) => (
                    <MenuRow key={name} onClick={() => onSubmit(name)}>
                        {name}
                    </MenuRow>
                ))}
            </div>
        </MenuCard>
    );
}

export default TagMenu;
