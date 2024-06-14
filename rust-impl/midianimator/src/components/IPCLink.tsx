import { useState } from "react";
import { useStateContext } from "../contexts/StateContext";

function IPCLink() {
    const { state, setState } = useStateContext();

    const [menuShown, setMenuShown] = useState(false);

    function openMenu() {
        setMenuShown(!menuShown);
        // openMenu ? setOpenMenu(false) : setOpenMenu(true);
    }

    function disconnect() {
        console.log("disconnect button pushed");
    }

    function connectDisconnectButton() {
        if (state.connected) {
            return <button className="bg-transparent font-semibold py-2 px-4 border border-black rounded" onClick={disconnect}>Disconnect</button>
        }
    }

    const menu = (
        <div className={`flex items-center flex-col ipc-content absolute top-[10%] bg-white border-black border-[1px] ${menuShown ? "" : "hidden"}`}>
            <p>{state.connected ? "blenderfile.blend" : "Disconnected. Please connect on Blender to start."}</p>
            {connectDisconnectButton()  /* want to only show the disconnect button when a client is connected */} 
        </div>
    );

    return (
        <div onClick={openMenu} className="flex items-center pl-5 pr-5 ml-auto">
            <div className="blender size-6 p-[inherit]" />
            <div className={`mac-traffic-light ${state.connected ? "green" : "red"} ml-1 m-[inherit]`}></div>
            <span className="mr-1 helvetica font-bold text-[8px] p-[inherit]">{state.connected ? "CONNECTED" : "DISCONNECTED"}</span>
            <div className={`ipc-content absolute top-[10%] bg-white border-black border-[1px] ${menuShown ? "" : "hidden"}`}>{state.connected ? "blenderfile.blend" : "Disconnected"}</div>
            {menu}
        </div>
    );
}

export default IPCLink;
