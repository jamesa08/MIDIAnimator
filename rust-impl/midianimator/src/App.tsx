import TabBar from "./components/TabBar";
import ToolBar from "./components/ToolBar";
import Panel from "./components/Panel";
import StatusBar from "./components/StatusBar";
import UnsavedChangesModal from "./components/UnsavedChangesModal";
import { type CSSProperties, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import { useStateContext } from "./contexts/StateContext";
import NodeGraph from "./components/NodeGraph";
import { NODE_DROP_EVENT } from "./utils/node";
import { takeGraph, takeState } from "./utils/graphOps";
import { PANELS, PANEL_DOCK_EVENT, PANEL_DRAG_EVENT, PANEL_DROP_EVENT, PANEL_NODE_DROP_EVENT, PANEL_TOGGLE_EVENT, Side, clientToScreen, dockSideAt, ensureDragGhostWindow, hidePanelWindow, panelSide, reshowPanelWindow, screenToClient, scrollbarWidth, showPanelWindow, withPoppedOut, PANEL_WIDTH } from "./utils/panels";

// height of a panel's window the first time it opens floating from the window menu, it's as wide as a docked panel
const FLOATING_HEIGHT = 360;

function App() {
    const { backEndState: backEndState, setBackEndState: setBackEndState, frontEndState: frontEndState, setFrontEndState: setFrontEndState } = useStateContext();

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

    // closing the main window asks to save unsaved changes first, the app quits once it's gone
    const [confirmClose, setConfirmClose] = useState(false);

    useEffect(() => {
        const unlisten = getCurrentWindow().onCloseRequested(async (event) => {
            event.preventDefault();
            if (await invoke<boolean>("has_unsaved_changes")) setConfirmClose(true);
            else getCurrentWindow().destroy();
        });
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    // a cancelled save dialog keeps the window open
    const saveAndClose = async () => {
        setConfirmClose(false);
        try {
            await invoke("save_project");
            getCurrentWindow().destroy();
        } catch (error) {
            if (error !== "Save cancelled") console.error("Save failed:", error);
        }
    };

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
            // the first time it floats, it goes inside the main window's right edge
            if (floating && !reshowPanelWindow(id)) {
                const content = document.querySelector(".content")?.getBoundingClientRect();
                if (!content) return;
                const width = PANEL_WIDTH + scrollbarWidth();
                const { x, y } = await clientToScreen(content.right - width - 260, content.top + 40);
                showPanelWindow(id, x, y, width, FLOATING_HEIGHT);
            }
        });
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    // docked panels float over the canvas in a column on each side, the canvas' own controls move in past them
    const ids = Object.keys(PANELS).map(Number);
    const dockedOn = (side: Side) => !frontEndState.sidesHidden.includes(side) && ids.some((id) => panelSide(frontEndState, id) === side && frontEndState.panelsShown.includes(id) && !frontEndState.panelsPoppedOut.includes(id));
    const docked = "calc(232px + var(--scrollbar-width, 0px))";
    const contentStyle = { "--panel-left": dockedOn("left") ? docked : "0px", "--panel-right": dockedOn("right") ? docked : "0px" } as CSSProperties;
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
            <div className="content relative flex flex-auto" style={contentStyle}>
                <div className="node-graph card flex-grow">
                    <div data-live-resize="vignette" className="canvas-vignette" />
                    <NodeGraph />
                </div>
                {(["left", "right"] as const).map((side) => (
                    <div key={side} className={`dock-column dock-width dock-${side}`} style={dockedOn(side) ? {} : { display: "none" }}>
                        {panelsOn(side).map((id) => (
                            <Panel key={id} id={String(id)} name={PANELS[id].name} />
                        ))}
                    </div>
                ))}
                {dockHover !== null && <div className={`dock-indicator dock-width card absolute inset-y-3 pointer-events-none z-50 bg-blue-500/20 border-2 border-blue-500 ${dockHover === "right" ? "right-3" : "left-3"}`} />}
            </div>
            <div className="foot flex-initial">
                <StatusBar event="Ready." />
            </div>
            {confirmClose && <UnsavedChangesModal onSave={saveAndClose} onDiscard={() => getCurrentWindow().destroy()} onCancel={() => setConfirmClose(false)} />}
        </div>
    );
}

export default App;
