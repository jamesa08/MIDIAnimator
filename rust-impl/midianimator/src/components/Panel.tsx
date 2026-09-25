import React, { useEffect, useRef } from "react";
import { useStateContext } from "../contexts/StateContext";
import { NODE_DROP_EVENT } from "../utils/node";
import { PANEL_DOCK_EVENT, PANEL_DRAG_EVENT, PANEL_DROP_EVENT, clientToScreen, ensurePanelWindow, inDockZone, sendToMain, showPanelWindow, windowMover, withPoppedOut } from "../utils/panels";
import PanelBody, { PanelNodeDrop } from "./PanelBody";

interface PanelProps {
    id: string;
    name: string;
}

// docked panel in the main window, pops out into its own window from the button or by dragging the header out
const Panel: React.FC<PanelProps> = ({ id, name }) => {
    const { frontEndState, setFrontEndState } = useStateContext();
    const ref = useRef<HTMLDivElement>(null);
    const panelId = Number(id);
    const poppedOut = frontEndState.panelsPoppedOut.includes(panelId);

    // warm up the floating window at the docked size so popping out is instant
    useEffect(() => {
        const rect = ref.current?.getBoundingClientRect();
        ensurePanelWindow(panelId, rect?.width || 240, rect?.height || 400);
    }, [panelId]);

    const popOut = (x: number, y: number, width: number, height: number) => {
        setFrontEndState((prev: any) => withPoppedOut(prev, panelId, true));
        return showPanelWindow(panelId, x, y, width, height);
    };

    // pops out in place, nudged so it reads as floating
    const popOutButton = async () => {
        if (!ref.current) return;
        const rect = ref.current.getBoundingClientRect();
        const { x, y } = await clientToScreen(rect.left, rect.top);
        popOut(x + 24, y + 24, rect.width, rect.height);
    };

    // tear the panel off by dragging its header out of the dock slot
    const startTearOff = (event: React.PointerEvent<HTMLDivElement>) => {
        if (event.button !== 0 || !ref.current) return;
        if ((event.target as HTMLElement).closest("button")) return;

        const rect = ref.current.getBoundingClientRect();
        const grabX = event.clientX - rect.left;
        const grabY = event.clientY - rect.top;
        let move: ((x: number, y: number) => void) | null = null;

        const handleMove = (e: PointerEvent) => {
            // stays docked until the cursor leaves the dock slot
            if (!move) {
                if (inDockZone(panelId, e.clientX, e.clientY)) return;
                move = windowMover(popOut(e.screenX - grabX, e.screenY - grabY, rect.width, rect.height));
            }
            move(e.screenX - grabX, e.screenY - grabY);
            sendToMain(PANEL_DRAG_EVENT, { id: panelId, screenX: e.screenX, screenY: e.screenY });
        };

        const cleanup = () => {
            document.body.style.cursor = "";
            window.removeEventListener("pointermove", handleMove);
            window.removeEventListener("pointerup", handleUp);
            window.removeEventListener("keydown", handleKey, true);
        };

        const handleUp = (e: PointerEvent) => {
            cleanup();
            if (move) sendToMain(PANEL_DROP_EVENT, { id: panelId, screenX: e.screenX, screenY: e.screenY });
        };

        // escape puts a torn off panel back
        const handleKey = (e: KeyboardEvent) => {
            if (e.key !== "Escape") return;
            e.stopPropagation();
            cleanup();
            if (move) sendToMain(PANEL_DOCK_EVENT, { id: panelId });
        };

        event.preventDefault();
        document.body.style.cursor = "grabbing";
        window.addEventListener("pointermove", handleMove);
        window.addEventListener("pointerup", handleUp);
        window.addEventListener("keydown", handleKey, true);
    };

    const handleNodeDrop = (drop: PanelNodeDrop) => {
        window.dispatchEvent(new CustomEvent(NODE_DROP_EVENT, { detail: drop }));
    };

    const shown = frontEndState.panelsShown.includes(panelId) && !poppedOut;

    return (
        <div ref={ref} className="panel w-60 select-none p-0" style={shown ? {} : { display: "none" }}>
            <div className="panel-header h-6 border-b border-black flex items-center pl-2 pr-2 text-sm" onPointerDown={startTearOff}>
                <span className="mr-auto">{name}</span>
                <button className="float-right" onClick={popOutButton}>
                    Popout
                </button>
            </div>
            <PanelBody id={panelId} onNodeDrop={handleNodeDrop} />
        </div>
    );
};

export default Panel;
