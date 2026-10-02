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
        <div className="status-bar card select-none">
            <div className="panel-header text-[11px] leading-none flex items-center px-3 pb-0.5 h-4">
                <div className="mr-auto">{event}</div>
                <div>MotionKeys {version} {hash}</div>
            </div>
        </div>
    );
}

export default StatusBar;
