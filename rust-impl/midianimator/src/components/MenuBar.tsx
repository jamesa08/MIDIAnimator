import MacTrafficLights from "./MacTrafficLights";
import Tab from "./Tab";
import IPCLink from "./IPCLink";

function MenuBar() {
    return (
        <div data-tauri-drag-region className="menu-bar border-b border-b-black flex h-10">
            {navigator.userAgent.includes("Mac OS") && <MacTrafficLights />}
            <Tab name="Placeholder" />
            <IPCLink />
        </div>
    );
}

export default MenuBar;
