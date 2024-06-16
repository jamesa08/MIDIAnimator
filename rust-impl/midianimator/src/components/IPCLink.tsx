import { useState } from "react";
import { useStateContext } from "../contexts/StateContext";
import { invoke } from "@tauri-apps/api/tauri";

declare global {
    interface String {
        toProperCase(): string;
    }
}

// why oh why is this not a built-in function in JS?
String.prototype.toProperCase = function () {
    return this.replace(/\w\S*/g, function (txt) {
        return txt.charAt(0).toUpperCase() + txt.substr(1).toLowerCase();
    });
};

function IPCLink() {
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [menuShown, setMenuShown] = useState(false);

    function openMenu() {
        setMenuShown(!menuShown);
    }

    function disconnect() {
        invoke("disconnect_alL_clients");
        console.log("disconnect button pushed");
    }

    function showWhenConnected() {
        if (state.connected) {
            return (
                <>
                    <p>{`${state.connected_application.toProperCase()} version ${state.connected_version}`}</p>
                    <p>{`${state.connected_file_name}`}</p>
                    <p>{`Port: ${state.port}`}</p>
                    <button className="bg-transparent font-semibold py-2 px-4 border border-black rounded" onClick={disconnect}>
                        Disconnect
                    </button>
                </>
            );
        }
    }

    const floatingPanel = (
        <div className={`flex items-center flex-col ipc-content fixed top-[10%] bg-white border-black border-[1px] ${menuShown ? "" : "hidden"}`}>
            <p>{state.connected ? "" : "Disconnected. Please connect on the 3D application to start."}</p>
            {showWhenConnected()}
        </div>
    );

    return (
        <div onClick={openMenu} className="flex items-center pl-5 pr-5 ml-auto">
            <div className={`${state.connected_application} size-6 p-[inherit]`} />
            <div className={`mac-traffic-light ${state.connected ? "green" : "red"} ml-1 m-[inherit]`}></div>
            <span className="mr-1 helvetica font-bold text-[8px] p-[inherit]">{state.connected ? "CONNECTED" : "DISCONNECTED"}</span>
            {floatingPanel}
        </div>
    );
}

export default IPCLink;
function useEffect(arg0: () => void, arg1: any[]) {
    throw new Error("Function not implemented.");
}

