import TabBar from "./components/TabBar";
import ToolBar from "./components/ToolBar";
import Panel from "./components/Panel";
import StatusBar from "./components/StatusBar";
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

import { useStateContext } from "./contexts/StateContext";
import NodeGraph from "./components/NodeGraph";
import { NODE_DROP_EVENT } from "./utils/node";
import { PANELS, PANEL_DOCK_EVENT, PANEL_DRAG_EVENT, PANEL_DROP_EVENT, PANEL_NODE_DROP_EVENT, ensureDragGhostWindow, hidePanelWindow, inDockZone, screenToClient, withPoppedOut } from "./utils/panels";

function App() {
    const { backEndState: backEndState, setBackEndState: setBackEndState, frontEndState: frontEndState, setFrontEndState: setFrontEndState } = useStateContext();

    useEffect(() => {
        invoke("log", { message: "App mounted, starting initialization..." });
        invoke("splash_progress", { message: "Initializing..." });
        invoke("splash_progress", { message: "Setting up state listeners..." });
        const stateListner = listen("update_state", (event: any) => {
            setBackEndState(event.payload);
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
            executionRunner.then((f) => f());
        };
    }, []);

    // floating panel being dragged over its dock slot
    const [dockHover, setDockHover] = useState<number | null>(null);

    // floating panels report drags in screen pixels, dock them when dropped on their slot
    useEffect(() => {
        ensureDragGhostWindow();

        const dock = (id: number) => {
            hidePanelWindow(id);
            setDockHover(null);
            setFrontEndState((prev: any) => {
                const next = withPoppedOut(prev, id, false);
                return next.panelsShown.includes(id) ? next : { ...next, panelsShown: [...next.panelsShown, id] };
            });
        };

        const overDockZone = async ({ id, screenX, screenY }: any) => {
            const { x, y } = await screenToClient(screenX, screenY);
            return inDockZone(id, x, y);
        };

        const dragListener = listen(PANEL_DRAG_EVENT, async (event: any) => {
            const over = await overDockZone(event.payload);
            setDockHover(over ? event.payload.id : null);
        });

        const dropListener = listen(PANEL_DROP_EVENT, async (event: any) => {
            if (await overDockZone(event.payload)) dock(event.payload.id);
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

    return (
        <div className="wrapper w-screen h-screen overflow-hidden flex flex-col">
            <div className="head flex-initial">
                <TabBar />
                <ToolBar />
            </div>
            <div className="content relative flex flex-auto ">
                <Panel id="0" name="Nodes" />
                <div className="node-graph flex-grow border-black border-l border-r">
                    <NodeGraph />
                </div>
                <div className="ml-auto flex">
                    <Panel id="1" name="Properties" />
                </div>
                {dockHover !== null && <div className={`dock-indicator absolute inset-y-0 w-60 pointer-events-none z-50 bg-blue-500/20 border-2 border-blue-500 ${PANELS[dockHover]?.side === "right" ? "right-0" : "left-0"}`} />}
            </div>
            <div className="foot flex-initial">
                <StatusBar event="Ready." />
            </div>
        </div>
    );
}

export default App;
