import TabBar from "./components/TabBar";
import ToolBar from "./components/ToolBar";
import Panel from "./components/Panel";
import StatusBar from "./components/StatusBar";
import UnsavedChangesModal from "./components/UnsavedChangesModal";
import { type CSSProperties, type PointerEvent as ReactPointerEvent, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import { useStateContext } from "./contexts/StateContext";
import NodeGraph from "./components/NodeGraph";
import { NODE_DROP_EVENT } from "./utils/node";
import { takeGraph, takeState } from "./utils/graphOps";
import { pushModal } from "./utils/keymap";
import { floatingPanels, useOpenFile, useSaveTab, withFloatingFrames } from "./utils/tabs";
import { PANELS, PANEL_DOCK_EVENT, PANEL_DRAG_EVENT, PANEL_DROP_EVENT, PANEL_NODE_DROP_EVENT, PANEL_TOGGLE_EVENT, Side, clientToScreen, dockSideAt, dockWidth, ensureDragGhostWindow, hidePanelWindow, panelSide, reshowPanelWindow, screenToClient, scrollbarWidth, showPanelWindow, withPoppedOut, PANEL_WIDTH } from "./utils/panels";

// height of a panel's window the first time it opens floating from the window menu, it's as wide as a docked panel
const FLOATING_HEIGHT = 360;
// how much of the content a side's docked panels can take up
const DOCK_MAX_FRACTION = 0.4;
// how far past the narrowest width a dock column's edge is dragged before the side hides
const DOCK_HIDE_DETENT = 80;
// sent to the main window by the file menu (src-tauri/src/ui/menu.rs), payload "open", "save" or "save_as"
const FILE_EVENT = "menu-file";

// shows a floating panel's window: at `frame`, where it was last, or the first time inside the main window's right edge
async function showFloating(id: number, frame?: { x: number; y: number; width: number; height: number }) {
    if (frame) {
        await showPanelWindow(id, frame.x, frame.y, frame.width, frame.height);
        return;
    }
    if (reshowPanelWindow(id)) return;
    const content = document.querySelector(".content")?.getBoundingClientRect();
    if (!content) return;
    const width = PANEL_WIDTH + scrollbarWidth();
    const { x, y } = await clientToScreen(content.right - width - 260, content.top + 40);
    await showPanelWindow(id, x, y, width, FLOATING_HEIGHT);
}

function App() {
    const { backEndState: backEndState, setBackEndState: setBackEndState, frontEndState: frontEndState, setFrontEndState: setFrontEndState, setTabLayout } = useStateContext();
    const saveTab = useSaveTab();
    const openFile = useOpenFile();

    useEffect(() => {
        invoke("log", { message: "App mounted, starting initialization..." });
        invoke("splash_progress", { message: "Initializing..." });
        invoke("splash_progress", { message: "Setting up state listeners..." });
        // a graph older than the one shown (an edit came back first) isn't taken
        const stateListner = listen("update_state", (event: any) => {
            setBackEndState((s: any) => takeState(s, event.payload));
        });
        const graphListener = listen("graph_changed", (event: any) => {
            setBackEndState((s: any) => takeGraph(s, event.payload));
        });

        const executionRunner = listen("execute_function", (event: any) => {
            invoke(event.payload["function"], event.payload["args"]).then((res: any) => {});
        });

        invoke("splash_progress", { message: "Launching..." });
        // tell the backend we're ready & get the initial state
        invoke("ready").then((res: any) => {
            if (res !== null) {
                setBackEndState(res);
            }
            // close the splash screen once the app is ready
            invoke("close_splashscreen");
        });

        return () => {
            stateListner.then((f) => f());
            graphListener.then((f) => f());
            executionRunner.then((f) => f());
        };
    }, []);

    // the unsaved changes prompt while the window is closing, it resolves to what was picked
    const [closePrompt, setClosePrompt] = useState<((answer: "save" | "discard" | "cancel") => void) | null>(null);
    const answerClose = (answer: "save" | "discard" | "cancel") => {
        closePrompt?.(answer);
        setClosePrompt(null);
    };

    // closing the main window asks about each tab with unsaved changes first (showing it), the app quits once it's gone.
    // cancelling, or cancelling a save dialog, keeps the window open
    useEffect(() => {
        const unlisten = getCurrentWindow().onCloseRequested(async (event) => {
            event.preventDefault();
            const { unsaved_tabs } = await invoke<{ unsaved_tabs: string[] }>("get_project_status");
            for (const id of unsaved_tabs) {
                await invoke("switch_active_instance", { id });
                const answer = await new Promise<"save" | "discard" | "cancel">((resolve) => setClosePrompt(() => resolve));
                if (answer === "cancel") return;
                if (answer === "save") {
                    try {
                        await saveTab(id);
                    } catch (error) {
                        if (error !== "Save cancelled") console.error("Save failed:", error);
                        return;
                    }
                }
            }
            getCurrentWindow().destroy();
        });
        return () => {
            unlisten.then((f) => f());
        };
    }, [saveTab]);

    // the file menu acts on the tab on screen
    useEffect(() => {
        const unlisten = getCurrentWebviewWindow().listen<string>(FILE_EVENT, async ({ payload }) => {
            if (payload === "open") return openFile();
            try {
                await saveTab(undefined, payload === "save_as");
            } catch (error) {
                if (error !== "Save cancelled") console.error("Save failed:", error);
            }
        });
        return () => {
            unlisten.then((f) => f());
        };
    }, [saveTab, openFile]);

    // each tab has floating panels of its own: switching keeps where the old tab's are, then shows the new tab's where
    // it had them and hides the rest. switches are done in order
    const tab: string | undefined = backEndState.active_tab;
    const layoutByTab = useRef<Record<string, any>>({});
    if (tab) layoutByTab.current[tab] = frontEndState;
    const shownTab = useRef<string | null>(null);
    const switching = useRef(Promise.resolve());
    useEffect(() => {
        if (!tab || tab === shownTab.current) return;
        const previous = shownTab.current;
        shownTab.current = tab;
        switching.current = switching.current.then(async () => {
            const left = previous ? layoutByTab.current[previous] : null;
            if (previous && left) setTabLayout(previous, await withFloatingFrames(left));
            const layout = layoutByTab.current[tab];
            const floating = floatingPanels(layout);
            for (const id of Object.keys(PANELS).map(Number)) {
                if (floating.includes(id)) await showFloating(id, layout.floating?.[id]);
                else hidePanelWindow(id);
            }
        });
        switching.current.catch((e) => console.error(`Error switching floating panels: ${e}`));
    }, [tab]);

    // the dock slot a floating panel is being dragged over
    const [dockHover, setDockHover] = useState<Side | null>(null);

    // floating panels report drags in screen pixels, dock them on the side they're dropped on
    useEffect(() => {
        ensureDragGhostWindow();

        // docks at the bottom of `side` (the side it was last docked on when not given), showing that side if it was collapsed
        const dock = (id: number, side?: Side) => {
            hidePanelWindow(id);
            setDockHover(null);
            setFrontEndState((prev: any) => {
                const next = withPoppedOut(prev, id, false);
                const to = side ?? panelSide(prev, id);
                return { ...next, panelsShown: [...next.panelsShown.filter((p: number) => p !== id), id], panelSides: { ...next.panelSides, [id]: to }, sidesHidden: next.sidesHidden.filter((s: Side) => s !== to) };
            });
        };

        const sideUnder = async ({ screenX, screenY }: any) => {
            const { x, y } = await screenToClient(screenX, screenY);
            return dockSideAt(x, y);
        };

        const dragListener = listen(PANEL_DRAG_EVENT, async (event: any) => setDockHover(await sideUnder(event.payload)));

        const dropListener = listen(PANEL_DROP_EVENT, async (event: any) => {
            const side = await sideUnder(event.payload);
            if (side) dock(event.payload.id, side);
            else setDockHover(null);
        });

        const dockListener = listen(PANEL_DOCK_EVENT, (event: any) => dock(event.payload.id));

        // node dragged out of a floating nodes panel, hand it to the graph in client pixels
        const nodeDropListener = listen(PANEL_NODE_DROP_EVENT, async (event: any) => {
            const { screenX, screenY, ...drop } = event.payload;
            const { x, y } = await screenToClient(screenX, screenY);
            window.dispatchEvent(new CustomEvent(NODE_DROP_EVENT, { detail: { ...drop, clientX: x, clientY: y } }));
        });

        return () => {
            dragListener.then((f) => f());
            dropListener.then((f) => f());
            dockListener.then((f) => f());
            nodeDropListener.then((f) => f());
        };
    }, []);

    useEffect(() => {
        // FIXME temporary
        console.log("Frontend state updated:", frontEndState);
    }, [frontEndState]);

    // the window menu opens a closed panel the way it was (floating, or docked on its side) and closes an open one
    const frontEndRef = useRef(frontEndState);
    frontEndRef.current = frontEndState;
    useEffect(() => {
        const unlisten = getCurrentWebviewWindow().listen<number>(PANEL_TOGGLE_EVENT, async ({ payload: id }) => {
            const state = frontEndRef.current;
            const floating = state.panelsPoppedOut.includes(id);
            if (state.panelsShown.includes(id)) {
                if (floating) hidePanelWindow(id);
                setFrontEndState((prev: any) => ({ ...prev, panelsShown: prev.panelsShown.filter((p: number) => p !== id) }));
                return;
            }
            setFrontEndState((prev: any) => ({ ...prev, panelsShown: [...prev.panelsShown, id], sidesHidden: floating ? prev.sidesHidden : prev.sidesHidden.filter((s: Side) => s !== panelSide(prev, id)) }));
            // where this tab last had it
            if (floating) showFloating(id, state.floating?.[id]);
        });
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    // docked panels float over the canvas in a column on each side, the canvas' own controls move in past them
    const ids = Object.keys(PANELS).map(Number);
    const dockedOn = (side: Side) => !frontEndState.sidesHidden.includes(side) && ids.some((id) => panelSide(frontEndState, id) === side && frontEndState.panelsShown.includes(id) && !frontEndState.panelsPoppedOut.includes(id));
    const docked = (side: Side) => (dockedOn(side) ? `calc(var(--dock-${side}) + 8px + var(--scrollbar-width, 0px))` : "0px");
    const contentStyle = { "--dock-left": `${dockWidth(frontEndState, "left")}px`, "--dock-right": `${dockWidth(frontEndState, "right")}px`, "--panel-left": docked("left"), "--panel-right": docked("right") } as CSSProperties;

    // dragging a dock column's inner edge resizes that side. the width is set straight on the content while dragging and
    // kept in the layout once it's let go, cancelling (escape) puts it back. dragging on past the narrowest width hides the
    // side like the toolbar's collapse buttons, it keeps the width it had for when it's shown again
    const contentRef = useRef<HTMLDivElement>(null);
    const startDockResize = (side: Side, event: ReactPointerEvent<HTMLDivElement>) => {
        const content = contentRef.current;
        const column = event.currentTarget.parentElement;
        if (event.button !== 0 || !content || !column) return;

        const startX = event.clientX;
        const startWidth = dockWidth(frontEndState, side);
        const startPanel = content.style.getPropertyValue(`--panel-${side}`);
        const maxWidth = Math.max(PANEL_WIDTH, content.clientWidth * DOCK_MAX_FRACTION);
        let width = startWidth;
        let hidden = false;
        const setWidth = (w: number) => content.style.setProperty(`--dock-${side}`, `${w}px`);
        const setHidden = (hide: boolean) => {
            hidden = hide;
            column.style.display = hide ? "none" : "";
            content.style.setProperty(`--panel-${side}`, hide ? "0px" : startPanel);
        };
        const endModal = pushModal("drag", {
            cancel: () => {
                cleanup();
                setHidden(false);
                setWidth(startWidth);
            },
        });

        const handleMove = (e: PointerEvent) => {
            const dx = side === "left" ? e.clientX - startX : startX - e.clientX;
            if (hidden !== startWidth + dx < PANEL_WIDTH - DOCK_HIDE_DETENT) setHidden(!hidden);
            if (hidden) return;
            width = Math.round(Math.min(maxWidth, Math.max(PANEL_WIDTH, startWidth + dx)));
            setWidth(width);
        };

        const cleanup = () => {
            endModal();
            document.body.style.cursor = "";
            window.removeEventListener("pointermove", handleMove);
            window.removeEventListener("pointerup", handleUp);
        };

        const handleUp = () => {
            cleanup();
            if (hidden) {
                setWidth(startWidth);
                setFrontEndState((prev: any) => ({ ...prev, sidesHidden: [...prev.sidesHidden.filter((s: Side) => s !== side), side] }));
                return;
            }
            setFrontEndState((prev: any) => ({ ...prev, dockWidths: { ...prev.dockWidths, [side]: width } }));
        };

        event.preventDefault();
        document.body.style.cursor = "ew-resize";
        window.addEventListener("pointermove", handleMove);
        window.addEventListener("pointerup", handleUp);
    };
    // a side's panels in the order they were docked
    const panelsOn = (side: Side) => {
        const order = (id: number) => (frontEndState.panelsShown.includes(id) ? frontEndState.panelsShown.indexOf(id) : ids.length + id);
        return ids.filter((id) => panelSide(frontEndState, id) === side).sort((a, b) => order(a) - order(b));
    };

    return (
        <div data-live-resize="window" className="wrapper w-screen h-screen overflow-hidden flex flex-col">
            <div data-tauri-drag-region className="head flex-initial">
                {/* the tabs sit on the window, the active one opens down into the toolbar's card */}
                <TabBar />
                <div className="card head-card">
                    <ToolBar />
                </div>
            </div>
            <div ref={contentRef} className="content relative flex flex-auto" style={contentStyle}>
                <div className="node-graph card flex-grow" data-keymap-area="node_editor">
                    <div data-live-resize="vignette" className="canvas-vignette" />
                    <NodeGraph />
                </div>
                {(["left", "right"] as const).map((side) => (
                    <div key={side} className={`dock-column dock-width dock-${side}`} style={dockedOn(side) ? {} : { display: "none" }}>
                        {panelsOn(side).map((id) => (
                            <Panel key={id} id={String(id)} name={PANELS[id].name} />
                        ))}
                        <div className="dock-resizer" onPointerDown={(e) => startDockResize(side, e)} />
                    </div>
                ))}
                {dockHover !== null && <div className={`dock-indicator dock-width dock-${dockHover} card absolute inset-y-3 pointer-events-none z-50 bg-blue-500/20 border-2 border-blue-500 ${dockHover === "right" ? "right-3" : "left-3"}`} />}
            </div>
            <div className="foot flex-initial">
                <StatusBar event="Ready." />
            </div>
            {closePrompt && <UnsavedChangesModal onSave={() => answerClose("save")} onDiscard={() => answerClose("discard")} onCancel={() => answerClose("cancel")} />}
        </div>
    );
}

export default App;
