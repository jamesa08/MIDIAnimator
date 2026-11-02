import { useContext } from "react";
import { TabContext } from "../../utils/graphOps";
import { PATH_SEP } from "../../utils/groups";
import { useScopePath } from "../../contexts/GroupContext";
import { NodeTiming, duration, useConsoleSettings, useNodeTiming } from "../../utils/console";

// how long a node took in its tab's last run, over its top left corner while the profiler is on. yellow from the medium
// time on and red from the slow one (settings.json console.profiler_medium_ms and profiler_slow_ms), dimmed when it came
// from the memo. a node that didn't run (a writer a realtime run skipped) shows nothing
function NodeTime({ id }: { id?: string }) {
    const settings = useConsoleSettings();
    const tab = useContext(TabContext);
    const scope = useScopePath();
    const timing = useNodeTiming(tab, scope ? `${scope}${PATH_SEP}${id}` : (id ?? ""));
    if (!settings.profiler || !id || !timing || timing.status === "skipped") return null;
    return <div className={`node-time${speedClass(timing, settings.profiler_medium_ms, settings.profiler_slow_ms)}`}>{duration(timing.ms)}</div>;
}

// the time's color: dimmed from the memo, else by how long it took
function speedClass(timing: NodeTiming, medium: number, slow: number) {
    if (timing.cached) return " console-debug";
    if (timing.ms >= slow) return " console-error";
    if (timing.ms >= medium) return " console-warn";
    return "";
}

export default NodeTime;
