import { useEffect, useMemo, useRef, useState } from "react";
import { NodeEntry } from "../../utils/nodeEntries";
import { useModal } from "../../utils/keymap";
import { CATEGORY_LABELS, CATEGORY_ORDER } from "../../styles";
import { MenuCard, MenuPanel, MenuRow, MenuSearch } from "../GlassMenu";

// a category's place in the nodes panel's order, ones it doesn't know go last
const categoryIndex = (category: string) => {
    const index = CATEGORY_ORDER.indexOf(category);
    return index < 0 ? CATEGORY_ORDER.length : index;
};

// the add menu, like blender's: the nodes that can be added by category, each category's nodes to its side. typing
// searches them all, enter adds the first. opened with shift+a (or from a dropped link) at the cursor, and from the
// toolbar's add button under it
function NodeAddMenu({ entries, x, y, onSelect, onClose }: { entries: NodeEntry[]; x: number; y: number; onSelect: (key: string) => void; onClose: () => void }) {
    const [search, setSearch] = useState("");
    const [category, setCategory] = useState<string | null>(null);
    // the hovered category row's top, its nodes open level with it
    const [categoryTop, setCategoryTop] = useState(0);
    const searchRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        searchRef.current?.focus();
    }, []);

    const categories = useMemo(() => [...new Set(entries.map((entry) => entry.category))].sort((a, b) => categoryIndex(a) - categoryIndex(b)), [entries]);
    const typed = search.trim().toLowerCase();
    const matches = typed ? entries.filter((entry) => entry.label.toLowerCase().includes(typed) || entry.key.toLowerCase().includes(typed)) : [];

    // takes the keys while it's open, typing still goes to the search
    useModal(
        "add_menu",
        {
            confirm: () => {
                if (matches.length > 0) onSelect(matches[0].key);
            },
            cancel: onClose,
        },
        true,
        { passthrough: true }
    );

    const aside = !typed && category && (
        <MenuPanel className="glass-submenu w-56" style={{ top: categoryTop }}>
            {entries
                .filter((entry) => entry.category === category)
                .map((entry) => (
                    <MenuRow key={entry.key} onClick={() => onSelect(entry.key)}>
                        {entry.label}
                    </MenuRow>
                ))}
        </MenuPanel>
    );

    return (
        <MenuCard x={x} y={y} className="w-52" aside={aside}>
            <MenuSearch inputRef={searchRef} value={search} onChange={setSearch} />
            {typed
                ? matches.map((entry) => (
                      <MenuRow key={entry.key} detail={CATEGORY_LABELS[entry.category] ?? entry.category} onClick={() => onSelect(entry.key)}>
                          {entry.label}
                      </MenuRow>
                  ))
                : categories.map((name) => (
                      <MenuRow
                          key={name}
                          detail="›"
                          active={category === name}
                          onMouseEnter={(e) => {
                              setCategory(name);
                              setCategoryTop(e.currentTarget.offsetTop);
                          }}
                      >
                          {CATEGORY_LABELS[name] ?? name}
                      </MenuRow>
                  ))}
        </MenuCard>
    );
}

export default NodeAddMenu;
