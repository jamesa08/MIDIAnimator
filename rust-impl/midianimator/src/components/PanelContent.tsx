import React, { useEffect } from "react";
import { useParams } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { PANELS, PANEL_DOCK_EVENT, PANEL_DRAG_EVENT, PANEL_DROP_EVENT, PANEL_NODE_DROP_EVENT, sendToMain, windowMover } from "../utils/panels";
import PanelBody, { PanelNodeDrop } from "./PanelBody";

// popped out panel window, drag the header to move it and drop it on its dock slot to dock it back
const PanelContent: React.FC = () => {
    const { id } = useParams<{ id: string }>();
    const panelId = Number(id);

    useEffect(() => {
        // loaded, turn this into an invisible palette ready to pop out
        invoke("panel_window_init");

        // closing a palette (cmd+w) docks it, the window itself is kept around
        const unlisten = getCurrentWindow().onCloseRequested((event) => {
            event.preventDefault();
            sendToMain(PANEL_DOCK_EVENT, { id: panelId });
        });
        return () => {
            unlisten.then((f) => f());
        };
    }, [panelId]);

    // the window has no title bar, the header moves it
    const startMove = (event: React.PointerEvent<HTMLDivElement>) => {
        if (event.button !== 0) return;
        if ((event.target as HTMLElement).closest("button")) return;

        // no decorations so client pixels are window pixels
        const grabX = event.clientX;
        const grabY = event.clientY;
        const move = windowMover(Promise.resolve(getCurrentWindow()));

        const handleMove = (e: PointerEvent) => {
            move(e.screenX - grabX, e.screenY - grabY);
            sendToMain(PANEL_DRAG_EVENT, { id: panelId, screenX: e.screenX, screenY: e.screenY });
        };

        const handleUp = (e: PointerEvent) => {
            window.removeEventListener("pointermove", handleMove);
            window.removeEventListener("pointerup", handleUp);
            sendToMain(PANEL_DROP_EVENT, { id: panelId, screenX: e.screenX, screenY: e.screenY });
        };

        event.preventDefault();
        window.addEventListener("pointermove", handleMove);
        window.addEventListener("pointerup", handleUp);
    };

    // nodes dropped outside this window go to the graph in the main window
    const handleNodeDrop = ({ clientX, clientY, ...drop }: PanelNodeDrop) => {
        sendToMain(PANEL_NODE_DROP_EVENT, drop);
    };

    return (
        <div className="panel-window w-screen h-screen flex flex-col overflow-hidden select-none bg-white">
            <div className="panel-header h-6 flex-none border-b border-black flex items-center pl-2 pr-2 text-sm" onPointerDown={startMove}>
                <span className="mr-auto">{PANELS[panelId]?.name}</span>
                <button onClick={() => sendToMain(PANEL_DOCK_EVENT, { id: panelId })}>Dock</button>
            </div>
            <div className="flex-auto overflow-y-auto">
                <PanelBody id={panelId} onNodeDrop={handleNodeDrop} ghostWindow />
            </div>
        </div>
    );
};

export default PanelContent;
