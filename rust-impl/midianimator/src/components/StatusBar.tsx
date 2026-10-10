import {invoke} from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useStateContext } from "../contexts/StateContext";

const plural = (count: number, word: string) => `${count.toLocaleString()} ${word}${count === 1 ? "" : "s"}`;

// what the last write to Blender did, see LastWrite in src-tauri/src/state/mod.rs
export function lastWriteMessage(write: any): string | undefined {
    if (!write) return undefined;
    if (write.error != null) return "Scene Writer failed";
    if (write.written === 0) return "Nothing to write";
    return `Wrote ${plural(write.keyframes, "keyframe")} to ${plural(write.objects, "object")} in ${write.ms} ms`;
}

function StatusBar({ event }: { event: string }) {
    // the last write's message stays until the next one, each write fades it in again
    const write = useStateContext().backEndState?.last_write;
    const message = lastWriteMessage(write);

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
                <div key={write?.seq} className={`mr-auto${message ? " status-fade-in" : ""}`}>{message ?? event}</div>
                <div>MotionKeys {version} {hash}</div>
            </div>
        </div>
    );
}

export default StatusBar;
