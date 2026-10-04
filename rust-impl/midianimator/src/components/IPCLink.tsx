import { useState } from "react";
import { createPortal } from "react-dom";
import { useStateContext } from "../contexts/StateContext";
import { invoke } from "@tauri-apps/api/core";
import SceneDiffModal from "./SceneDiffModal";

declare global {
    interface String {
        toProperCase(): string;
    }
}

String.prototype.toProperCase = function () {
    return this.replace(/\w\S*/g, function (txt) {
        return txt.charAt(0).toUpperCase() + txt.slice(1).toLowerCase();
    });
};

function IPCLink() {
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [menuShown, setMenuShown] = useState(false);
    const [showDiffModal, setShowDiffModal] = useState(false);
    const [sceneDiff, setSceneDiff] = useState(null);

    function openMenu() {
        setMenuShown(!menuShown);
    }

    function disconnect() {
        console.log("disconnect button pushed");
    }

    const handleValidate = async () => {
        try {
            const diff: any = await invoke("check_scene_changes");
            setSceneDiff(diff);
            setShowDiffModal(true);
        } catch (error) {
            console.error("Validation failed:", error);
            alert(`Validation failed: ${error}`);
        }
    };

    const handleAccept = async () => {
        try {
            await invoke("accept_scene_changes");
            setShowDiffModal(false);
            setSceneDiff(null);
            // State will update via backend's update_state() call
        } catch (error) {
            console.error("Accept failed:", error);
            alert(`Failed to accept changes: ${error}`);
        }
    };

    const handleReject = async () => {
        try {
            await invoke("reject_scene_changes");
            setShowDiffModal(false);
            setSceneDiff(null);
            alert("Changes rejected. Staying in paused mode with original scene data.");
        } catch (error) {
            console.error("Reject failed:", error);
        }
    };

    // links Blender to the tab on screen, it checks the scene first (see go_live in src-tauri/src/state/mod.rs)
    const goLive = () => invoke("go_live", { id: state.active_tab }).catch((error) => console.error("Go live failed:", error));
    const activeLinked = (state.tabs ?? []).some((tab: any) => tab.id === state.active_tab && tab.linked);

    function showWhenConnected() {
        if (state.connected) {
            return (
                <>
                    <p>{`${state.connected_application.toProperCase()} version ${state.connected_version}`}</p>
                    <p>{`${state.connected_file_name}`}</p>
                    <p>{`Port: ${state.port}`}</p>
                    {state.execution_paused && (
                        <>
                            <p className="text-yellow-600 font-bold mt-2">⚠️ Execution paused - scene data needs validation</p>
                            <button className="bg-yellow-500 font-semibold py-2 px-4 border border-black rounded mt-2" onClick={handleValidate}>
                                Validate Scene Data
                            </button>
                        </>
                    )}
                    {!activeLinked && (
                        <button className="bg-transparent font-semibold py-2 px-4 border border-black rounded" onClick={goLive}>
                            Go Live
                        </button>
                    )}
                    <button className="bg-transparent font-semibold py-2 px-4 border border-black rounded" onClick={disconnect}>
                        Disconnect
                    </button>
                </>
            );
        }
    }

    // Determine status color and text
    const getStatus = () => {
        if (!state.connected) {
            return { color: "red", text: "DISCONNECTED" };
        }
        if (state.execution_paused) {
            return { color: "yellow", text: "PAUSED" };
        }
        return { color: "green", text: "CONNECTED" };
    };

    const status = getStatus();

    const floatingPanel = (
        <div className={`flex items-center flex-col ipc-content fixed top-[36px] right-1 max-w-[320px] z-[1100] wrap-anywhere bg-white border-black border-[1px] ${menuShown ? "" : "hidden"}`}>
            <p>{state.connected ? "" : "Disconnected. Please connect on the 3D application to start."}</p>
            {showWhenConnected()}
        </div>
    );

    return (
        <>
            <div data-tauri-drag-region onClick={openMenu} className="ipc-link flex items-center gap-2 pl-3 pr-5 ml-auto">
                <div className={`${state.connected_application} size-6 ${state.connected ? "" : "hidden"}`} />
                <div className={`mac-traffic-light ${status.color}`}></div>
                <span className="helvetica font-bold text-[8px] translate-y-px">{status.text}</span>
                {/* on the body so it sits above the toolbar and panels, the tab strip is its own stacking context */}
                {createPortal(floatingPanel, document.body)}
            </div>

            {showDiffModal && <SceneDiffModal diff={sceneDiff} onAccept={handleAccept} onReject={handleReject} onClose={() => setShowDiffModal(false)} />}
        </>
    );
}

export default IPCLink;
