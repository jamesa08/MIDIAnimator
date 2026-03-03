import React, { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import nodeTypes from "../nodes/NodeTypes";
import { ReactFlowProvider } from "@xyflow/react";
import { WebviewWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { useStateContext } from "../contexts/StateContext";
import { safeWindowPosition } from "../utils/window";

interface PanelProps {
    id: string;
    name: string;
}

const Panel: React.FC<PanelProps> = ({ id, name }) => {
    const navigate = useNavigate();
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

        const webview = new WebviewWindow(id, {
            url: `/#/panel/${id}`,
            title: name,
            width: w,
            height: h,
            resizable: true,
            x: x,
            y: y,
        });

        webview.once("tauri://created", () => {
            console.log("Created new window");
        });

        webview.once("tauri://error", (e: any) => {
            console.error(`Error creating new window ${e.payload}`);
        });

        navigate(`/#/panel/${id}`);
    };

    const ScaledNodeWrapper: React.FC<{ Node: any }> = ({ Node }) => {
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
            <div ref={nodeRef} className="node-container">
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
                        <ScaledNodeWrapper key={key} Node={value} />
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
