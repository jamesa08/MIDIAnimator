import MacTrafficLights from "./MacTrafficLights";
import Tab from "./Tab";
import IPCLink from "./IPCLink";
import AddTabButton from "./AddTabButton";

function TabBar() {
    return (
        <div data-tauri-drag-region className="tab-bar border-b border-b-black flex h-7">
            {navigator.userAgent.includes("Mac OS") && <MacTrafficLights />}
            <div data-tauri-drag-region className="flex min-w-0 w-full">
                <Tab name="untitled" />
                <AddTabButton />
            </div>
            <IPCLink />
        </div>
    );
}

export default TabBar;
