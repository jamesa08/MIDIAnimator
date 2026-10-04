import { useState, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Reorder, AnimatePresence, motion } from "framer-motion";
import MacTrafficLights from "./MacTrafficLights";
import IPCLink from "./IPCLink";
import AddTabButton from "./AddTabButton";
import UnsavedChangesModal from "./UnsavedChangesModal";
import { useStateContext } from "../contexts/StateContext";
import { useSaveTab } from "../utils/tabs";

// a tab from the backend (TabInfo in src-tauri/src/state/mod.rs): its name, its file (null until it's saved, a saved tab
// is named after its file), and whether Blender is linked to it
interface Tab {
    id: string;
    label: string;
    path: string | null;
    linked: boolean;
}

type ProjectStatus = { name: string; unsaved: boolean; unsaved_tabs: string[] };

// the tabs, the backend owns them: each is a file of its own, like an instance of the app. adding, closing, switching,
// renaming and moving one are backend commands (src-tauri/src/state/mod.rs)
function TabBar() {
    const { backEndState: state } = useStateContext();
    const saveTab = useSaveTab();
    const tabs: Tab[] = state.tabs ?? [];
    const activeId: string = state.active_tab ?? "";

    // the order on screen, ahead of the backend's while a tab is dragged
    const [order, setOrder] = useState<Tab[]>(tabs);
    const tabsKey = JSON.stringify(tabs);
    useEffect(() => setOrder(tabs), [tabsKey]);
    const orderRef = useRef(order);
    orderRef.current = order;

    // tabs with unsaved changes get a star, closing one asks first
    const [unsavedTabs, setUnsavedTabs] = useState<string[]>([]);
    useEffect(() => {
        const setStatus = (status: ProjectStatus) => setUnsavedTabs(status.unsaved_tabs ?? []);
        invoke<ProjectStatus>("get_project_status").then(setStatus);
        const unlisten = listen<ProjectStatus>("project_status", (event) => setStatus(event.payload));
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    // the tab being renamed and its name so far
    const [editing, setEditing] = useState<string | null>(null);
    const [draft, setDraft] = useState("");

    // the tab waiting on the unsaved changes prompt
    const [confirmClose, setConfirmClose] = useState<string | null>(null);

    const addTab = () => invoke("create_instance");
    const switchTab = (id: string) => {
        if (id !== activeId) invoke("switch_active_instance", { id });
    };

    // the last tab closes the window instead, which asks to save the project
    const latest = useRef({ tabs, activeId, unsavedTabs });
    latest.current = { tabs, activeId, unsavedTabs };
    const closeTab = (id: string) => {
        const { tabs, unsavedTabs } = latest.current;
        if (tabs.length <= 1) getCurrentWindow().close();
        else if (unsavedTabs.includes(id)) setConfirmClose(id);
        else invoke("close_instance", { id });
    };

    const discardAndClose = () => {
        if (confirmClose) invoke("close_instance", { id: confirmClose });
        setConfirmClose(null);
    };

    // a cancelled save dialog keeps the tab
    const saveAndClose = async () => {
        const id = confirmClose;
        setConfirmClose(null);
        if (!id) return;
        try {
            await saveTab(id);
            invoke("close_instance", { id });
        } catch (error) {
            if (error !== "Save cancelled") console.error("Save failed:", error);
        }
    };

    // only a tab that hasn't been saved, a saved one is named after its file
    const startRename = (tab: Tab) => {
        if (tab.path) return;
        setEditing(tab.id);
        setDraft(tab.label);
    };
    const finishRename = (commit: boolean) => {
        if (editing && commit && draft.trim() !== "") invoke("rename_instance", { id: editing, label: draft });
        setEditing(null);
    };

    // a dragged tab lands where it was let go
    const dropTab = (id: string) => {
        const index = orderRef.current.findIndex((t) => t.id === id);
        if (index !== latest.current.tabs.findIndex((t) => t.id === id)) invoke("move_instance", { id, index });
    };

    // close (cmd/ctrl+w, Close Window in the menu) closes the active tab, or the window on the last one. new tab is the
    // menu's (ui/menu.rs)
    useEffect(() => {
        const closeListener = listen("close-tab", () => closeTab(latest.current.activeId));
        return () => {
            closeListener.then((f) => f());
        };
    }, []);

    return (
        <div data-tauri-drag-region className="tab-bar border-b border-b-black flex h-9">
            {navigator.userAgent.includes("Mac OS") && <MacTrafficLights />}

            <div className="flex min-w-0 w-full overflow-hidden pl-3 pr-[32px]">
                <Reorder.Group data-tauri-drag-region as="div" axis="x" values={order} onReorder={setOrder} className="flex w-full" layoutScroll>
                    <AnimatePresence mode="popLayout" initial={false}>
                        {order.map((tab, i) => {
                            const isLast = i === order.length - 1;
                            const unsaved = unsavedTabs.includes(tab.id);
                            return (
                                <Reorder.Item
                                    as="div"
                                    key={tab.id}
                                    value={tab}
                                    dragListener={editing !== tab.id}
                                    onDragEnd={() => dropTab(tab.id)}
                                    initial={{ opacity: 0 }}
                                    animate={{ opacity: 1 }}
                                    exit={{ opacity: 0 }}
                                    layout="position"
                                    transition={{
                                        opacity: { duration: 0.15, ease: "easeOut" },
                                    }}
                                    transformTemplate={(transformProps, generated) => generated.replace(/translateX\(([^)]+)\)/, (_, v) => `translateX(${Math.round(parseFloat(v))}px)`)}
                                    className="flex items-center h-full shrink max-w-[280px]"
                                    // sized to the name, at least an even share of the strip up to 160px, pushing later tabs over
                                    style={{ overflow: "visible", position: "relative", minWidth: `max(80px, min(160px, ${100 / order.length}%))` }}
                                    onClick={() => switchTab(tab.id)}
                                >
                                    <div className={`relative flex items-center gap-1 px-3 h-full text-sm cursor-pointer select-none w-full min-w-0 tab-item ${tab.id === activeId ? "active bg-white" : "bg-zinc-100 hover:bg-zinc-100"}`}>
                                        {/* Blender is linked to this tab: live while it's connected, offline while it's away */}
                                        {tab.linked && <span className={`tab-dot ${state.connected ? "green" : "offline"}`} />}
                                        {editing === tab.id ? (
                                            <input
                                                className="tab-rename flex-1 min-w-0 bg-transparent outline-none"
                                                value={draft}
                                                autoFocus
                                                onFocus={(e) => e.target.select()}
                                                onChange={(e) => setDraft(e.target.value)}
                                                onBlur={() => finishRename(true)}
                                                onClick={(e) => e.stopPropagation()}
                                                onKeyDown={(e) => {
                                                    e.stopPropagation();
                                                    if (e.key === "Enter") finishRename(true);
                                                    else if (e.key === "Escape") finishRename(false);
                                                }}
                                            />
                                        ) : (
                                            <span className="truncate flex-1" onDoubleClick={() => startRename(tab)}>
                                                {`${tab.label}${unsaved ? "*" : ""}`}
                                            </span>
                                        )}
                                        <motion.button
                                            onClick={(e) => {
                                                e.stopPropagation();
                                                closeTab(tab.id);
                                            }}
                                            whileHover={{ scale: 1.2 }}
                                            whileTap={{ scale: 0.9 }}
                                            className="ml-1 text-zinc-400 hover:text-black"
                                        >
                                            {/* drawn instead of a × character, which sits low in its line and won't center on the tab */}
                                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={2} stroke="currentColor" className="size-3 block">
                                                <path strokeLinecap="round" d="M6.5 6.5l11 11m0-11l-11 11" />
                                            </svg>
                                        </motion.button>
                                    </div>
                                    {isLast && (
                                        <div
                                            onClick={(e) => {
                                                e.stopPropagation();
                                                addTab();
                                            }}
                                            className="shrink-0"
                                            style={{ position: "absolute", left: "100%", top: "var(--tab-inset)", bottom: 1 }}
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
            {/* on the body so it sits above the toolbar and panels, the tab strip is its own stacking context */}
            {confirmClose && createPortal(<UnsavedChangesModal onSave={saveAndClose} onDiscard={discardAndClose} onCancel={() => setConfirmClose(null)} />, document.body)}
        </div>
    );
}

export default TabBar;
