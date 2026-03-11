import MacTrafficLights from "./MacTrafficLights";
import Tab from "./Tab";
import IPCLink from "./IPCLink";

function MenuBar() {
    return (
        <div data-tauri-drag-region className="menu-bar border-b border-b-black flex h-7">
            {navigator.userAgent.includes("Mac OS") && <MacTrafficLights />}
            <div data-tauri-drag-region className="flex min-w-0 w-full">
                <Tab name="untitled" />
            </div>
            <IPCLink />
        </div>
    );
}

export default MenuBar;
