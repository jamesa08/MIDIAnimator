import { useState } from "react";
import { useStateContext } from "../contexts/StateContext";
import { invoke } from "@tauri-apps/api/tauri";
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
        <div className={`flex items-center flex-col ipc-content fixed top-[10%] bg-white border-black border-[1px] ${menuShown ? "" : "hidden"}`}>
            <p>{state.connected ? "" : "Disconnected. Please connect on the 3D application to start."}</p>
            {showWhenConnected()}
        </div>
    );

    return (
        <>
            <div onClick={openMenu} className="flex items-center pl-5 pr-5 ml-auto">
                <div className={`${state.connected_application} size-6 p-[inherit]`} />
                <div className={`mac-traffic-light ${status.color} ml-1 m-[inherit]`}></div>
                <span className="mr-1 helvetica font-bold text-[8px] p-[inherit]">{status.text}</span>
                {floatingPanel}
            </div>

            {showDiffModal && <SceneDiffModal diff={sceneDiff} onAccept={handleAccept} onReject={handleReject} onClose={() => setShowDiffModal(false)} />}
        </>
    );
}

export default IPCLink;
