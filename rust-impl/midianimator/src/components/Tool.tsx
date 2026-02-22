import React from "react";
import { useStateContext } from "../contexts/StateContext";
import { invoke } from "@tauri-apps/api/tauri";

function Tool({ type }: { type: string }) {
    const { backEndState, setBackEndState } = useStateContext(); // Add this import at top

    var icon;
    if (type == "run") {
        icon = (
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="size-6">
                <path strokeLinecap="round" strokeLinejoin="round" d="M5.25 5.653c0-.856.917-1.398 1.667-.986l11.54 6.347a1.125 1.125 0 0 1 0 1.972l-11.54 6.347a1.125 1.125 0 0 1-1.667-.986V5.653Z" />
            </svg>
        );
    } else if (type == "save") {
        icon = (
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="size-6">
                <path strokeLinecap="round" strokeLinejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5M16.5 12L12 16.5m0 0L7.5 12m4.5 4.5V3" />
            </svg>
        );
    } else if (type == "load") {
        icon = (
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="size-6">
                <path strokeLinecap="round" strokeLinejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5" />
            </svg>
        );
    } else if (type == "collapse-left") {
        icon = <img src="src/collapse-left.png" alt="collapse-left" className="size-6 h-5" />;
    } else if (type == "collapse-right") {
        icon = <img src="src/collapse-right.png" alt="collapse-right" className="size-6 h-5" />;
    }

    return (
        <div
            className="toolbar-button flex items-center pr-1 pl-1"
            onClick={async () => {
                if (type == "run") {
                    if (backEndState.execution_paused) {
                        alert("Execution paused: Validate scene data first");
                        return;
                    }
                    invoke("execute_graph", { realtime: false });
                } else if (type == "save") {
                    try {
                        const path = await invoke<string>("save_project");
                        console.log("Saved to:", path);
                    } catch (error) {
                        console.error("Save failed:", error);
                    }
                } else if (type == "load") {
                    try {
                        const newState = await invoke("load_project");
                        setBackEndState(newState);
                    } catch (error) {
                        if (error !== "Load cancelled") {
                            console.error("Load failed:", error);
                        }
                    }
                }
            }}
        >
            {icon}
        </div>
    );
}

export default Tool;
