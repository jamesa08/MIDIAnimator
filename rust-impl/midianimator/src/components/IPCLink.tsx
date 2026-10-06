import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useStateContext } from "../contexts/StateContext";
import { invoke } from "@tauri-apps/api/core";
import SceneDiffModal from "./SceneDiffModal";
import { useModal } from "../utils/keymap";

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

    const linkRef = useRef<HTMLDivElement>(null);
    const labelRef = useRef<HTMLSpanElement>(null);
    const popoverRef = useRef<HTMLDivElement>(null);
    // the popover's left edge and its notch's offset from it, the notch points at the status label
    const [placement, setPlacement] = useState({ left: 0, top: 0, notch: 0 });

    function openMenu() {
        setMenuShown(!menuShown);
    }

    // centered under the label, kept 8px inside the window
    useLayoutEffect(() => {
        if (!menuShown) return;
        const place = () => {
            const label = labelRef.current!.getBoundingClientRect();
            const width = popoverRef.current?.offsetWidth ?? 0;
            const center = label.left + label.width / 2;
            const left = Math.max(8, Math.min(center - width / 2, window.innerWidth - 8 - width));
            setPlacement({ left, top: label.bottom + 12, notch: center - left });
        };
        place();
        window.addEventListener("resize", place);
        return () => window.removeEventListener("resize", place);
    }, [menuShown]);

    // closes on a click anywhere else
    useEffect(() => {
        if (!menuShown) return;
        const close = (event: PointerEvent) => {
            const target = event.target as Node;
            if (!popoverRef.current?.contains(target) && !linkRef.current?.contains(target)) setMenuShown(false);
        };
        window.addEventListener("pointerdown", close, true);
        return () => window.removeEventListener("pointerdown", close, true);
    }, [menuShown]);

    useModal("dialog", { cancel: () => setMenuShown(false) }, menuShown, { passthrough: true });

    const disconnect = () => invoke("disconnect").catch((error) => console.error("Disconnect failed:", error));

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

    const button = "h-7 w-full border border-black rounded-md text-sm hover:bg-zinc-100";

    function showWhenConnected() {
        return (
            <>
                <div className="text-sm leading-6 wrap-anywhere">
                    <p>{`${state.connected_application.toProperCase()} version ${state.connected_version}`}</p>
                    <p>{`${state.connected_file_name}`}</p>
                    <p>{`Port: ${state.port}`}</p>
                </div>
                {state.execution_paused && (
                    <>
                        <p className="text-xs text-yellow-600 mt-1">⚠️ Execution paused - scene data needs validation</p>
                        <button className={`${button} mt-2`} onClick={handleValidate}>
                            Validate Scene Data
                        </button>
                    </>
                )}
                {!activeLinked && (
                    <button className={`${button} mt-2`} onClick={goLive}>
                        Go Live
                    </button>
                )}
                <button className={`${button} mt-2`} onClick={disconnect}>
                    Disconnect
                </button>
            </>
        );
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

    // a callout under the status label, its notch is a rotated square drawn over the top border
    const floatingPanel = (
        <div ref={popoverRef} style={{ left: placement.left, top: placement.top }} className="fixed w-64 z-[1100] bg-white border border-black font-[Arial,sans-serif] select-none">
            <div style={{ left: placement.notch }} className="absolute -top-[6px] size-[11px] -translate-x-1/2 rotate-45 bg-white border-l border-t border-black" />
            <div className="relative px-3 py-3">
                {state.connected ? (
                    showWhenConnected()
                ) : (
                    <p className="text-sm text-center py-4">
                        Disconnected.
                        <br />
                        Please connect on Blender to start.
                    </p>
                )}
            </div>
        </div>
    );

    return (
        <>
            <div ref={linkRef} data-tauri-drag-region onClick={openMenu} className="ipc-link flex items-center gap-2 pl-3 pr-5 ml-auto">
                <div className={`${state.connected_application} size-6 ${state.connected ? "" : "hidden"}`} />
                <div className={`mac-traffic-light ${status.color}`}></div>
                <span ref={labelRef} className="helvetica font-bold text-[8px] translate-y-px">
                    {status.text}
                </span>
                {state.connected && (
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={3} stroke="currentColor" className="size-2 -ml-1 translate-y-px">
                        <path strokeLinecap="round" strokeLinejoin="round" d="M5 9l7 7 7-7" />
                    </svg>
                )}
            </div>

            {/* on the body so it sits above the toolbar and panels, the tab strip is its own stacking context. outside the
                link so clicks inside it don't toggle the menu */}
            {menuShown && createPortal(floatingPanel, document.body)}

            {showDiffModal && <SceneDiffModal diff={sceneDiff} onAccept={handleAccept} onReject={handleReject} onClose={() => setShowDiffModal(false)} />}
        </>
    );
}

export default IPCLink;
