import React, { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import nodeTypes from "../nodes/NodeTypes";
import { ReactFlowProvider } from "@xyflow/react";
import { WebviewWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

interface PanelProps {
    id: string;
    name: string;
}

const Panel: React.FC<PanelProps> = ({ id, name }) => {
    const navigate = useNavigate();

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

        setupListener();
    }, []);

    const createWindow = (event: React.MouseEvent<HTMLButtonElement>) => {
        const webview = new WebviewWindow(id, {
            url: `/#/panel/${id}`,
            title: name,
            width: 400,
            height: 300,
            resizable: true,
            x: event.screenX,
            y: event.screenY,
        });

        webview.once("tauri://created", () => {
            console.log("Created new window");
        });

        webview.once("tauri://error", (e: any) => {
            console.error(`Error creating new window ${e.payload}`);
        });

        navigate(`/#/panel/${id}`);
    };

    function windowIfNodes() {
        if (name == "Nodes") {
            return (
                <ReactFlowProvider>
                    {Object.entries(nodeTypes).map(([key, value]) => {
                        const Node: any = value;
                        return <Node data="preview" />;
                    })}
                </ReactFlowProvider>
            );
        }
    }

    return (
        <div className="panel w-60 select-none">
            <div className="panel-header h-8 border-b border-black flex items-center pl-2 pr-2">
                <span className="mr-auto">{name}</span>
                <button className="float-right" onClick={createWindow}>
                    Popout
                </button>
            </div>
            {windowIfNodes()}
        </div>
    );
};

export default Panel;
