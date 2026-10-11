import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { NodeEntry } from "../../utils/nodeEntries";
import { useModal } from "../../utils/keymap";

// shift+a menu, searches the nodes that can be added
function NodeAddMenu({ isOpen, entries, onClose, onSelect, position }: { isOpen: boolean; entries: NodeEntry[]; onClose: () => void; onSelect: (key: string) => void; position: { x: number; y: number } }) {
    const [search, setSearch] = useState("");
    const searchInputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        if (!isOpen) {
            setSearch("");
        }
    }, [isOpen]);

    const filteredEntries = entries.filter((entry) => entry.label.toLowerCase().includes(search.toLowerCase()) || entry.key.toLowerCase().includes(search.toLowerCase()));

    useEffect(() => {
        if (isOpen && searchInputRef.current) {
            searchInputRef.current.focus();
        }
    }, [isOpen]);

    // takes the keys while it's open, typing still goes to the search
    useModal(
        "add_menu",
        {
            confirm: () => {
                if (filteredEntries.length > 0) onSelect(filteredEntries[0].key);
            },
            cancel: onClose,
        },
        isOpen,
        { passthrough: true }
    );

    if (!isOpen) return null;

    // on the body so it sits above the floating panels, the canvas is its own stacking context
    return createPortal(
        <div
            style={{
                position: "fixed",
                left: position.x,
                top: position.y,
            }}
            className="bg-[#2a2a2a] border border-[#444] rounded w-[250px] max-h-[400px] z-[1000] flex flex-col"
            onMouseDown={(e) => e.stopPropagation()}
        >
            <input ref={searchInputRef} type="text" value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Search nodes..." className="px-2 py-1 bg-[#1a1a1a] border-0 border-b border-[#444] text-white outline-none text-[13px]" />
            <div style={{ overflowY: "auto", maxHeight: "350px" }}>
                {filteredEntries.map((entry) => (
                    <div
                        key={entry.key}
                        onClick={() => onSelect(entry.key)}
                        style={{
                            backgroundColor: "transparent",
                        }}
                        className="px-2 py-1 cursor-pointer text-white text-[13px] flex justify-between"
                        onMouseEnter={(e) => {
                            e.currentTarget.style.backgroundColor = "#4a7ba7";
                        }}
                        onMouseLeave={(e) => {
                            e.currentTarget.style.backgroundColor = "transparent";
                        }}
                    >
                        <span>{entry.label}</span>
                        {entry.nodeType === "group" && <span className="text-[#8ab4d8]">group</span>}
                    </div>
                ))}
            </div>
        </div>,
        document.body
    );
}

export default NodeAddMenu;
