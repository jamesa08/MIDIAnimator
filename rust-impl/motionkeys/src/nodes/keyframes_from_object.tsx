import "@xyflow/react/dist/base.css";
import { useCallback, useEffect, useRef } from "react";
import { useStore, useUpdateNodeInternals } from "@xyflow/react";
import BaseNode from "./BaseNode";
import CurvePreview from "../components/graph/CurvePreview";
import { useNodeSpec } from "../utils/nodeEntries";
import { useStateContext } from "../contexts/StateContext";
import { useGroupContext } from "../contexts/GroupContext";
import { Op, useGraphOps, useSetInputs } from "../utils/graphOps";

// an animated channel of the object, see Channel in src-tauri/src/graph/executors/animation.rs
type Channel = { id: string; group: string; name: string };

// object channels show just their name, the others their group too
const OBJECT_GROUP = "Object";
const channelLabel = (c: Channel) => (c.group === OBJECT_GROUP ? c.name : `${c.group} › ${c.name}`);

function keyframes_from_object({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const { scopeId, editable } = useGroupContext();
    const { apply } = useGraphOps(scopeId);
    const updateNodeInternals = useUpdateNodeInternals();
    const nodeData = useNodeSpec("keyframes_from_object");
    const { backEndState: state } = useStateContext();

    // Derive everything from state and data directly
    const executedInputs = state?.executed_inputs?.[id];
    const executedResults = state?.executed_results?.[id];

    // a mistyped connection can hand us a non-array, so don't trust the shape
    const objectGroups: any[] = Array.isArray(executedInputs?.["object_groups"]) ? executedInputs["object_groups"] : [];
    const objectGroupNames: string[] = objectGroups.map((g: any) => g?.name);

    const selectedGroupName: string = data.inputs?.object_group_name || objectGroupNames[0] || "";

    const objectNames: string[] = objectGroups.find((g: any) => g?.name === selectedGroupName)?.objects?.map?.((o: any) => o?.name) ?? [];

    const selectedObjectName: string = data.inputs?.object_name || objectNames[0] || "";

    // a picked name the scene doesn't have (anymore) stays listed, so the dropdown shows it and picking a real one changes it
    const withSelected = (names: string[], selected: string) => (!selected || names.includes(selected) ? names : [selected, ...names]);
    const groupOptions = withSelected(objectGroupNames, selectedGroupName);
    const objectOptions = withSelected(objectNames, selectedObjectName);

    // the picked channels, one output each
    const channels: string[] = Array.isArray(data.inputs?.channels) ? data.inputs.channels : [];

    // the object's channels from the last run that got here. a failed run (or one that stopped before this node) keeps them,
    // so the dropdowns keep their names and the node can still be edited
    const lastAvailable = useRef<Channel[]>([]);
    if (Array.isArray(executedResults?.available_channels)) lastAvailable.current = executedResults.available_channels;
    const available = lastAvailable.current;

    // a picked channel the object doesn't have (anymore) shows its id
    const channelFor = (channel: string): Channel => available.find((c) => c.id === channel) ?? { id: channel, group: OBJECT_GROUP, name: channel };

    // outputs something is connected to keep their socket too, e.g. a project opened with this node failing.
    // stored edges are reversed, `target`/`targetHandle` is the node giving the value and its output
    const connectedOutputs = useCallback((s: any) => s.edges.flatMap((e: any) => (e.target === id && e.targetHandle ? [`${e.id}\t${e.targetHandle}\t${e.source}\t${e.sourceHandle}`] : [])).join("\n"), [id]);
    const connections: { edge: string; output: string; toNode: string; toInput: string }[] = useStore(connectedOutputs)
        .split("\n")
        .filter(Boolean)
        .map((line: string) => {
            const [edge, output, toNode, toInput] = line.split("\t");
            return { edge, output, toNode, toInput };
        });
    const unpicked = [...new Set(connections.map((c) => c.output))].filter((output) => output !== "dyn_output" && !channels.includes(output));

    const outputs = [...channels.map((channel) => ({ id: channel, name: channelLabel(channelFor(channel)), data_type: "Array<Keyframe>" })), ...unpicked.map((output) => ({ id: output, name: output, data_type: "Array<Keyframe>" }))];

    // sockets moved between rows without the node changing size, so react flow has to measure them again
    const outputsKey = outputs.map((o) => o.id).join("\n");
    useEffect(() => {
        updateNodeInternals(id);
    }, [id, outputsKey, updateNodeInternals]);

    useEffect(() => {
        console.log("state.executed_results[id] changed:", state?.executed_results?.[id]);
        // filled in automatically, not an undo step
        if (selectedGroupName && selectedGroupName !== data.inputs?.object_group_name) {
            setInputs(id, { object_group_name: selectedGroupName, object_name: selectedObjectName }, { commitToHistory: false });
        }
    }, [selectedGroupName, selectedObjectName]);

    // MARK: - Channels

    // sets the picked channels, the connections of a channel that changed move with its row and a removed one's go
    const pickChannels = (next: string[], moved: Record<string, string | null>) => {
        if (!editable) return;
        const ops: Op[] = [{ op: "set_inputs", node: id, inputs: { channels: next } }];
        const changed = connections.filter((c) => c.output in moved);
        if (changed.length > 0) ops.push({ op: "delete", edges: changed.map((c) => c.edge) });
        for (const c of changed) {
            const to = moved[c.output];
            if (to) ops.push({ op: "connect", from_node: id, from_output: to, to_node: c.toNode, to_input: c.toInput });
        }
        apply(ops).catch((e) => console.error(`set channels on ${id}: ${e}`));
    };

    const addChannel = (channel: string) => {
        pickChannels([...channels, channel], {});
    };

    const changeChannel = (index: number, channel: string) => {
        const next = [...channels];
        next[index] = channel;
        pickChannels(next, { [channels[index]]: channel });
    };

    const removeChannel = (index: number) => {
        pickChannels(
            channels.filter((_, i) => i !== index),
            { [channels[index]]: null }
        );
    };

    // the object's channels as dropdown options, by group
    const channelOptions = (options: Channel[]) =>
        [...new Set(options.map((c) => c.group))].map((group) => (
            <optgroup key={group} label={group}>
                {options
                    .filter((c) => c.group === group)
                    .map((c) => (
                        <option key={c.id} value={c.id}>
                            {c.name}
                        </option>
                    ))}
            </optgroup>
        ));

    // a row's dropdown: the object's channels, without the ones other rows picked
    const channelSelect = (channel: string, index: number) => {
        const options = available.filter((c) => c.id === channel || !channels.includes(c.id));
        if (!options.some((c) => c.id === channel)) options.unshift(channelFor(channel));
        return (
            <div className="channel-row" style={{ width: "100%" }}>
                <button className="channel-remove nodrag nopan" onClick={() => removeChannel(index)}>
                    ×
                </button>
                <select className="nodrag nopan" value={channel} onChange={(e) => changeChannel(index, e.target.value)}>
                    {channelOptions(options)}
                </select>
            </div>
        );
    };

    // the channels no row picked yet. picking one in the empty dropdown at the end adds its row, like a free socket
    const unpickedChannels = available.filter((c) => !channels.includes(c.id));
    const emptySelect = editable && unpickedChannels.length > 0 && (
        <div className="node-field channel-row">
            {/* keeps the dropdown in line with the rows' */}
            <button className="channel-remove" style={{ visibility: "hidden" }} tabIndex={-1}>
                ×
            </button>
            <select className="nodrag nopan" value="" onChange={(e) => addChannel(e.target.value)}>
                <option value="" disabled hidden></option>
                {channelOptions(unpickedChannels)}
            </select>
        </div>
    );

    // the old group's object isn't in the new one, so the object goes back to the new group's first
    const changeGroup = (groupName: string) => {
        const firstObject = objectGroups.find((g: any) => g?.name === groupName)?.objects?.[0]?.name ?? "";
        setInputs(id, { object_group_name: groupName, object_name: firstObject });
    };

    const objectGroupNameComponent = (
        <select className="node-field nodrag nopan" value={selectedGroupName} onChange={(e) => changeGroup(e.target.value)}>
            {groupOptions.length > 0 ? (
                groupOptions.map((name, i) => (
                    <option key={i} value={name}>
                        {name}
                    </option>
                ))
            ) : (
                <option value="">No ObjectGroup names found</option>
            )}
        </select>
    );

    const objectNameComponent = (
        <select className="node-field nodrag nopan" value={selectedObjectName} onChange={(e) => setInputs(id, { object_name: e.target.value })}>
            {objectOptions.length > 0 ? (
                objectOptions.map((name, i) => (
                    <option key={i} value={name}>
                        {name}
                    </option>
                ))
            ) : (
                <option value="">No Object names found</option>
            )}
        </select>
    );

    // the empty dropdown goes under the last output, or where the outputs would be
    const uiInject: any = {
        object_group_name: objectGroupNameComponent,
        object_name: objectNameComponent,
        [outputs.length > 0 ? outputs[outputs.length - 1].id : "dyn_output"]: emptySelect,
    };

    const hiddenHandles = {
        object_group_name: true,
        object_name: true,
        channels: true,
        dyn_output: true,
        available_channels: true,
    };

    const dynamicHandles: any = {
        outputs: outputs,
    };

    // each picked channel's name is its dropdown
    const labels = Object.fromEntries(channels.map((channel, i) => [channel, channelSelect(channel, i)]));

    // the picked channels' curves, under the inputs
    return (
        <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} dynamicHandles={dynamicHandles} labels={labels} data={data}>
            <CurvePreview outputs={executedResults} />
        </BaseNode>
    );
}

export default keyframes_from_object;
