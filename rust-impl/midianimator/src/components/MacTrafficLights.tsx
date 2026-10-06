import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

function MacTrafficLights() {
    // full screen has no traffic lights in the window, the tabs take their place
    const [fullscreen, setFullscreen] = useState(false);
    useEffect(() => {
        const window = getCurrentWindow();
        const check = () => window.isFullscreen().then(setFullscreen);
        check();
        // entering and leaving full screen resize the window
        const unlisten = window.onResized(check);
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    if (fullscreen) return null;
    return (
        <div data-tauri-drag-region className="mac-traffic-lights-container">
            <div className="mac-traffic-light red"></div>
            <div className="mac-traffic-light yellow"></div>
            <div className="mac-traffic-light green"></div>
        </div>
    );
}

export default MacTrafficLights;
