import {invoke} from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
function StatusBar({ event }: { event: string }) {
    
    const [version, setVersion] = useState("");
    const [hash, setHash] = useState("");
    useEffect(() => {
        invoke("get_build_info").then((res: any) => {
            const [version, hash] = res;
            setVersion(version);
            setHash(hash);
        });
    }, []);

    return (
        <div className="status-bar w-screen border-black border select-none">
            <div className="panel-header text-xs p-0.3 border-b border-black flex items-center pl-2 pr-2 h-4">
                <div className="text-xs mr-auto">{event}</div>
                <div className="text-xs">MotionKeys {version} {hash}</div>
            </div>
        </div>
    );
}

export default StatusBar;
