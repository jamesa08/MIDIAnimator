import MacTrafficLights from "./mac-traffic-lights";
import Tabs from "./Tabs";

function MenuBar() {
    return (
        <div className="menu-bar border-b border-b-black">
            <MacTrafficLights />
            <Tabs />
        </div>
    )
}

export default MenuBar;