import React, { useEffect, useRef, useState } from "react";
import nodeTypes from "../nodes/NodeTypes";
import { ReactFlowProvider } from "@xyflow/react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen } from "@tauri-apps/api/event";
import { useStateContext } from "../contexts/StateContext";
import { safeWindowPosition } from "../utils/window";
import { NODE_DROP_EVENT } from "../utils/node";

interface PanelProps {
    id: string;
    name: string;
}

const Panel: React.FC<PanelProps> = ({ id, name }) => {
    const { frontEndState, setFrontEndState } = useStateContext();
    const ref = useRef<HTMLDivElement>(null);


    useEffect(() => {
        const handleClick = (event: any) => {
            console.log(`Got ${JSON.stringify(event)} on window listener`);
        };

        const setupListener = async () => {
            try {
                const unlisten = await listen("clicked", handleClick);
                return () => {
                    unlisten();
                };
            } catch (error) {
                console.error("Failed to setup event listener:", error);
            }
        };

        if (ref.current) {
            const rect = ref.current.getBoundingClientRect();
            console.log("RECT", rect.width, rect.height, rect.top, rect.left);
        }

        setupListener();
    }, []);

    const createWindow = async (event: React.MouseEvent<HTMLButtonElement>) => {
        let w = 400;
        let h = 300;

        if (ref.current) {
            const rect = ref.current.getBoundingClientRect();
            w = rect.width
            h = rect.height
        }

        const { x, y } = await safeWindowPosition(event.screenX, event.screenY, w, h);

        // focus the popout if it's already open, creating it again would fail on the duplicate label
        const label = `panel-${id}`;
        const existing = await WebviewWindow.getByLabel(label);
        if (existing) {
            await existing.setFocus();
            return;
        }

        const webview = new WebviewWindow(label, {
            url: `/#/panel/${id}`,
            title: name,
            width: w,
            height: h,
            resizable: true,
            x: x,
            y: y,
            useHttpsScheme: true,
        });

        webview.once("tauri://created", () => {
            console.log("Created new window");
        });

        webview.once("tauri://error", (e: any) => {
            console.error(`Error creating new window ${e.payload}`);
        });
    };

    // drag a preview node out of the panel, the node graph adds it where it's released.
    // pointer events instead of html5 drag and drop, tauri's native drop handling swallows html5 drops
    const startNodeDrag = (event: React.PointerEvent<HTMLDivElement>, nodeType: string) => {
        if (event.button !== 0) return;
        const preview = event.currentTarget.querySelector(".node.preview") as HTMLElement | null;
        if (!preview) return;

        // previews are drawn at half scale, grab offset is kept in real node pixels
        const rect = preview.getBoundingClientRect();
        const scale = rect.width / preview.offsetWidth || 0.5;
        const grabX = event.clientX - rect.left;
        const grabY = event.clientY - rect.top;
        const startX = event.clientX;
        const startY = event.clientY;
        let ghost: HTMLElement | null = null;

        const moveGhost = (x: number, y: number) => {
            if (ghost) ghost.style.transform = `translate(${x - grabX}px, ${y - grabY}px) scale(${scale})`;
        };

        const handleMove = (e: PointerEvent) => {
            // small dead zone so a plain click doesn't start a drag
            if (!ghost && Math.hypot(e.clientX - startX, e.clientY - startY) < 4) return;
            if (!ghost) {
                // clone of the preview that follows the cursor
                ghost = preview.cloneNode(true) as HTMLElement;
                Object.assign(ghost.style, { position: "fixed", left: "0", top: "0", width: `${preview.offsetWidth}px`, margin: "0", opacity: "0.75", pointerEvents: "none", zIndex: "2000", transformOrigin: "top left", cursor: "grabbing" });
                document.body.appendChild(ghost);
                document.body.style.cursor = "grabbing";
            }
            moveGhost(e.clientX, e.clientY);
        };

        const cleanup = () => {
            ghost?.remove();
            document.body.style.cursor = "";
            window.removeEventListener("pointermove", handleMove);
            window.removeEventListener("pointerup", handleUp);
            window.removeEventListener("keydown", handleKey, true);
        };

        const handleUp = (e: PointerEvent) => {
            const dragged = ghost != null;
            cleanup();
            if (!dragged) return;
            window.dispatchEvent(new CustomEvent(NODE_DROP_EVENT, { detail: { nodeType, clientX: e.clientX, clientY: e.clientY, offsetX: grabX / scale, offsetY: grabY / scale } }));
        };

        // escape cancels the drag
        const handleKey = (e: KeyboardEvent) => {
            if (e.key !== "Escape") return;
            e.stopPropagation();
            cleanup();
        };

        event.preventDefault();
        window.addEventListener("pointermove", handleMove);
        window.addEventListener("pointerup", handleUp);
        window.addEventListener("keydown", handleKey, true);
    };

    const ScaledNodeWrapper: React.FC<{ Node: any; nodeType: string }> = ({ Node, nodeType }) => {
        const nodeRef = useRef<HTMLDivElement>(null);
        const [isMeasured, setIsMeasured] = useState(false);

        useEffect(() => {
            if (!nodeRef.current || isMeasured) return;

            const node = nodeRef.current.querySelector(".node.preview") as HTMLElement;
            if (!node) return;

            const observer = new MutationObserver(() => {
                const height = node.scrollHeight;

                if (height > 50) {
                    node.style.marginBottom = `-${height * 0.5}px`;
                    setIsMeasured(true);
                    observer.disconnect();
                }
            });

            observer.observe(node, {
                childList: true,
                subtree: true,
            });

            return () => observer.disconnect();
        }, [isMeasured]);

        return (
            <div ref={nodeRef} className="node-container" onPointerDown={(e) => startNodeDrag(e, nodeType)}>
                <Node data="preview" />
            </div>
        );
    };

    const renderNodesPanel = () => {
        if (name !== "Nodes") return null;

        return (
            <ReactFlowProvider>
                <div className="nodes-grid p-2">
                    {Object.entries(nodeTypes).map(([key, value]) => (
                        <ScaledNodeWrapper key={key} Node={value} nodeType={key} />
                    ))}
                </div>
            </ReactFlowProvider>
        );
    };

    return (
        <div ref={ref} className="panel w-60 select-none p-0" style={frontEndState.panelsShown.includes(Number(id)) ? {} : { display: "none" }}>
            <div className="panel-header h-6 border-b border-black flex items-center pl-2 pr-2 text-sm">
                <span className="mr-auto">{name}</span>
                <button className="float-right" onClick={createWindow}>
                    Popout
                </button>
            </div>
            {renderNodesPanel()}
        </div>
    );
};

export default Panel;
