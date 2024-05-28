import MacTrafficLights from "./mac-traffic-lights";
import Tab from "./Tab";
import IPCLink from "./IPCLink";

function MenuBar() {
    return (
        <div className="menu-bar border-b border-b-black flex h-10">
            {navigator.userAgent.includes("Mac OS") && <MacTrafficLights />}
            <Tab name="Areallylongnameforafilethatplaysmusic" />
            <Tab name="Areallylongnameforafilethatplaysmusic" />
            <Tab name="Areallylongnameforafilethatplaysmusic" />
            <Tab name="Areallylongnameforafilethatplaysmusic" />
            <IPCLink />
        </div>
    );
}

export default MenuBar;
