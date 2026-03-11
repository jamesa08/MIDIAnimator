import { useState } from "react";
import { Reorder, AnimatePresence, motion } from "framer-motion";
import MacTrafficLights from "./MacTrafficLights";
import IPCLink from "./IPCLink";
import AddTabButton from "./AddTabButton";

interface Tab {
    id: string;
    name: string;
}

let nextId = 1;
const makeTab = (name = "untitled"): Tab => ({ id: `tab-${nextId++}`, name });

function TabBar() {
    const [tabs, setTabs] = useState<Tab[]>([makeTab()]);
    const [activeId, setActiveId] = useState<string>(tabs[0].id);

    const addTab = () => {
        const tab = makeTab();
        setTabs((prev) => [...prev, tab]);
        setActiveId(tab.id);
    };

    const closeTab = (id: string) => {
        setTabs((prev) => {
            const next = prev.filter((t) => t.id !== id);
            if (next.length === 0) {
                const fresh = makeTab();
                setActiveId(fresh.id);
                return [fresh];
            }
            if (id === activeId) {
                const idx = prev.findIndex((t) => t.id === id);
                setActiveId(next[Math.min(idx, next.length - 1)].id);
            }
            return next;
        });
    };

    return (
        <div data-tauri-drag-region className="tab-bar border-b border-b-black flex h-7">
            {navigator.userAgent.includes("Mac OS") && <MacTrafficLights />}

            <div data-tauri-drag-region className="flex min-w-0 w-full overflow-hidden">
                <Reorder.Group axis="x" values={tabs} onReorder={setTabs} className="flex min-w-0" layoutScroll style={{ overflowX: "hidden" }}>
                    <AnimatePresence initial={false}>
                        {tabs.map((tab, i) => {
                            return (
                                <Reorder.Item key={tab.id} value={tab} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.08, ease: "easeOut" }} layoutTransition={{ type: "spring", stiffness: 500, damping: 35 }} className="shrink-0 max-w-[160px] min-w-[80px] h-full" onClick={() => setActiveId(tab.id)}>
                                    <div style={i !== 0 ? { marginLeft: "-1px" } : undefined} className={`relative flex items-center gap-1 px-3 h-full text-sm cursor-pointer select-none border-r border-l border-black ${tab.id === activeId ? "bg-white" : "bg-zinc-100 hover:bg-zinc-100"} `}>
                                        <span className="truncate flex-1">{tab.name}</span>
                                        <motion.button
                                            onClick={(e) => {
                                                e.stopPropagation();
                                                closeTab(tab.id);
                                            }}
                                            whileHover={{ scale: 1.2 }}
                                            whileTap={{ scale: 0.9 }}
                                            className="ml-1 text-zinc-400 hover:text-black leading-none"
                                        >
                                            ×
                                        </motion.button>
                                    </div>
                                </Reorder.Item>
                            );
                        })}{" "}
                    </AnimatePresence>
                </Reorder.Group>

                <AddTabButton onClick={addTab} />
            </div>

            <IPCLink />
        </div>
    );
}

export default TabBar;
