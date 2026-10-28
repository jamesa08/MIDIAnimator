import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Icon } from "../icons";
import { useModal } from "../utils/keymap";
import { TOOLBAR_ADD_EVENT, useEditorState } from "../utils/editorState";
import { NodeEntry } from "../utils/nodeEntries";
import { CATEGORY_LABELS, CATEGORY_ORDER } from "../styles";

// a category's place in the nodes panel's order, ones it doesn't know go last
const categoryIndex = (category: string) => {
    const index = CATEGORY_ORDER.indexOf(category);
    return index < 0 ? CATEGORY_ORDER.length : index;
};

// the toolbar's add button, like blender's add menu: the nodes that can be added in the graph on screen by category, each
// category's nodes to its side. typing searches them all, enter adds the first. the picked node is added under the cursor
// and grabbed (NodeGraphEditor)
function AddNodeMenu() {
    const { editable, entries } = useEditorState();
    const enabled = editable && !!entries;
    const [open, setOpen] = useState(false);
    const [search, setSearch] = useState("");
    const [category, setCategory] = useState<string | null>(null);
    const buttonRef = useRef<HTMLDivElement>(null);
    const searchRef = useRef<HTMLInputElement>(null);

    const close = useCallback(() => {
        setOpen(false);
        setSearch("");
        setCategory(null);
    }, []);

    // closes when clicked outside, or when the graph on screen can't take a node
    useEffect(() => {
        if (!open) return;
        window.addEventListener("mousedown", close);
        return () => window.removeEventListener("mousedown", close);
    }, [open, close]);
    useEffect(() => {
        if (!enabled) close();
    }, [enabled, close]);
    useEffect(() => {
        if (open) searchRef.current?.focus();
    }, [open]);

    const categories = useMemo(() => [...new Set((entries ?? []).map((entry) => entry.category))].sort((a, b) => categoryIndex(a) - categoryIndex(b)), [entries]);
    const typed = search.trim().toLowerCase();
    const matches = typed ? (entries ?? []).filter((entry) => entry.label.toLowerCase().includes(typed) || entry.key.toLowerCase().includes(typed)) : [];

    const pick = (entry: NodeEntry) => {
        close();
        window.dispatchEvent(new CustomEvent(TOOLBAR_ADD_EVENT, { detail: entry.key }));
    };

    // takes the keys while it's open, typing still goes to the search
    useModal(
        "add_menu",
        {
            confirm: () => {
                if (matches.length > 0) pick(matches[0]);
            },
            cancel: close,
        },
        open,
        { passthrough: true }
    );

    const row = "px-2 py-[3px] cursor-default hover:bg-zinc-100";
    const rect = open ? buttonRef.current?.getBoundingClientRect() : null;

    return (
        <>
            <div ref={buttonRef} className={`toolbar-button${enabled ? "" : " disabled"}`} data-tool="add_node" onMouseDown={(e) => e.stopPropagation()} onClick={() => (open ? close() : setOpen(true))}>
                <Icon name="add" className="tool-icon" />
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={3} stroke="currentColor" className="size-2">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M5 9l7 7 7-7" />
                </svg>
            </div>

            {/* on the body so it sits above the panels and the canvas, like the connection menu */}
            {open &&
                rect &&
                createPortal(
                    <div style={{ left: rect.left, top: rect.bottom + 4 }} className="fixed w-52 z-[1100] bg-white border border-black font-[Arial,sans-serif] text-[13px] select-none flex flex-col" onMouseDown={(e) => e.stopPropagation()}>
                        <input ref={searchRef} type="text" value={search} onChange={(e) => setSearch(e.target.value)} spellCheck={false} className="px-2 py-1 bg-white border-0 border-b border-black outline-none text-[13px]" />
                        {typed
                            ? matches.map((entry) => (
                                  <div key={entry.key} className={`${row} flex justify-between gap-2`} onClick={() => pick(entry)}>
                                      <span className="truncate">{entry.label}</span>
                                      <span className="opacity-50 flex-none">{CATEGORY_LABELS[entry.category] ?? entry.category}</span>
                                  </div>
                              ))
                            : categories.map((name) => (
                                  <div key={name} className={`${row} relative flex justify-between${category === name ? " bg-zinc-100" : ""}`} onMouseEnter={() => setCategory(name)}>
                                      <span>{CATEGORY_LABELS[name] ?? name}</span>
                                      <span className="opacity-50">›</span>
                                      {category === name && (
                                          <div className="absolute left-full -top-px w-56 bg-white border border-black flex flex-col">
                                              {(entries ?? [])
                                                  .filter((entry) => entry.category === name)
                                                  .map((entry) => (
                                                      <div key={entry.key} className={`${row} truncate`} onClick={() => pick(entry)}>
                                                          {entry.label}
                                                      </div>
                                                  ))}
                                          </div>
                                      )}
                                  </div>
                              ))}
                    </div>,
                    document.body
                )}
        </>
    );
}

export default AddNodeMenu;
