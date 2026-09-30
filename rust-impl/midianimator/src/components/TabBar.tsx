import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Reorder, AnimatePresence, useTransform, useMotionValue, motion } from "framer-motion";
import MacTrafficLights from "./MacTrafficLights";
import IPCLink from "./IPCLink";
import AddTabButton from "./AddTabButton";

interface Tab {
    id: string;
    name: string;
}

let nextId = 1;

const SPRING = { type: "spring", stiffness: 280, damping: 32, mass: 0.8 };

function TabBar() {
    const makeTab = (name = "untitled"): Tab => ({ id: `tab-${nextId++}`, name });

    const [tabs, setTabs] = useState<Tab[]>([makeTab()]);
    const [activeId, setActiveId] = useState<string>(tabs[0].id);
    const tabCount = useRef(tabs.length);
    tabCount.current = tabs.length;

    // the project's name, starred with unsaved changes, shown on the active tab
    const [project, setProject] = useState<{ name: string; unsaved: boolean } | null>(null);
    useEffect(() => {
        invoke<{ name: string; unsaved: boolean }>("get_project_status").then(setProject);
        const unlisten = listen<{ name: string; unsaved: boolean }>("project_status", (event) => setProject(event.payload));
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    const addTab = () => {
        const tab = makeTab();
        console.log("Adding tab", tab);
        setTabs((prev) => [...prev, tab]);
        setActiveId(tab.id);
    };
    

    const closeTab = (id: string) => {
        console.log("Closing tab", id);
        setTabs((prev) => {
            const next = prev.filter((t) => t.id !== id);
            if (next.length === 0) {
                const fresh = makeTab();
                setActiveId(fresh.id);
                return [fresh];
            }
            setActiveId((currentActive) => {
                console.log("currentActive", currentActive);
                console.log("next", next[next.length - 1].id);
                // if (currentActive !== id) return currentActive;
                return next[next.length - 1].id;
            });
            return next;
        });
    };

    useEffect(() => {
        console.log("Tabs changed", tabs);
        const handler = (e: KeyboardEvent) => {
            if (!(e.metaKey || e.ctrlKey)) return;
            if (e.key === "t") {
                e.preventDefault();
                addTab();
            }
        };
        window.addEventListener("keydown", handler);

        // cmd/ctrl+w (Close Window in the menu) closes the active tab, or the window on the last one
        const closeListener = listen("close-tab", () => {
            if (tabCount.current <= 1) {
                getCurrentWindow().close();
                return;
            }
            setActiveId((currentId) => {
                closeTab(currentId);
                return currentId;
            });
        });

        return () => {
            window.removeEventListener("keydown", handler);
            closeListener.then((f) => f());
        };
    }, []);

    return (
        <div data-tauri-drag-region className="tab-bar border-b border-b-black flex h-7">
            {navigator.userAgent.includes("Mac OS") && <MacTrafficLights />}

            <div className="flex min-w-0 w-full overflow-hidden pl-3 pr-[32px]">
                <Reorder.Group data-tauri-drag-region as="div" axis="x" values={tabs} onReorder={setTabs} className="flex w-full" layoutScroll>
                    <AnimatePresence mode="popLayout" initial={false}>
                        {tabs.map((tab, i) => {
                            const isLast = i === tabs.length - 1;
                            return (
                                <Reorder.Item
                                    as="div"
                                    key={tab.id}
                                    value={tab}
                                    initial={{ opacity: 0 }}
                                    animate={{ opacity: 1 }}
                                    exit={{ opacity: 0 }}
                                    layout
                                    transition={{
                                        opacity: { duration: 0.15, ease: "easeOut" },
                                    }}
                                    transformTemplate={(transformProps, generated) => generated.replace(/translateX\(([^)]+)\)/, (_, v) => `translateX(${Math.round(parseFloat(v))}px)`)}
                                    className={`flex items-center h-full ${isLast ? "w-[160px] min-w-[80px]" : "w-[160px] min-w-[80px]"}`}
                                    style={{ overflow: "visible", position: "relative" }}
                                    onClick={() => setActiveId(tab.id)}
                                >
                                    <div className={`relative flex items-center gap-1 px-3 h-full text-sm cursor-pointer select-none w-full tab-item ${tab.id === activeId ? "active bg-white" : "bg-zinc-100 hover:bg-zinc-100"}`}>
                                        <span className="truncate flex-1">{tab.id === activeId && project ? `${project.name}${project.unsaved ? "*" : ""}` : tab.name}</span>
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
                                    {isLast && (
                                        <div
                                            onClick={(e) => {
                                                e.stopPropagation();
                                                addTab();
                                            }}
                                            className="shrink-0"
                                            style={{ position: "absolute", left: "100%", top: 0, height: "100%" }}
                                        >
                                            <AddTabButton onClick={() => {}} />
                                        </div>
                                    )}
                                </Reorder.Item>
                            );
                        })}
                    </AnimatePresence>
                </Reorder.Group>
            </div>

            <IPCLink />
        </div>
    );
}

export default TabBar;
