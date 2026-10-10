# Nodes and Backend

## Adding a Node

A node is three pieces that share one id (e.g. `scene_writer`): a spec in `default_nodes.json`, a Rust function, and a UI component. The Scene Writer node is a small example of all three.

### 1. Spec (`src-tauri/src/configs/default_nodes.json`)

```json
{
    "id": "scene_writer",
    "name": "Scene Writer",
    "description": "Write keyframes to the Blender scene",
    "executor": "rust",
    "realtime": false,
    "handles": {
        "inputs": [
            {
                "id": "keyframes",
                "name": "Keyframes",
                "data_type": "HashMap<String, Array<BlendKeyframe>>",
                "description": "Keyframes per object from Evaluate Instrument."
            }
        ],
        "outputs": []
    }
}
```

- **`realtime`**: `true` runs the node on every graph edit. `false` only runs it on a full execute. Anything that writes to Blender must be `false`, as the realtime graph never writes.
- **`description`** is required on every handle, since the ML model reads them over MCP (`graph_test.rs` checks this).
- **`hidden: true`** on an input means it's set by a UI element in the node (e.g. a file picker), not by a connection. On an output, it's display-only.

### 2. Rust function (`src-tauri/src/graph/executors/*.rs`)

```rust
/// Node: scene_writer
///
/// inputs:
/// "keyframes": `HashMap<String, Array<BlendKeyframe>>`
///
/// outputs:
/// None, the result of the write is logged to the console
#[node_registry::node]
pub fn scene_writer(inputs: Inputs) -> NodeResult {
    let keyframes: serde_json::Value = inputs.get("keyframes")?;
    // ...
    Ok(Outputs::new())
}
```

- The function name must match the spec `id`. `build.rs` finds every `#[node_registry::node]` function and generates `src/node_registry.rs`, so there's nothing to register by hand.
- Read inputs with `inputs.get` (required, errors if missing), `inputs.opt` (`None` if missing) or `inputs.or_default`. They deserialize into any serde type and give a readable error on a wrong type.
- Set outputs with `outputs.set("handle_id", &value)?`. Keys must match the output handle ids.
- Return `Err(String)` for anything that goes wrong. The executor stores it on the node, stops the nodes after it, and reports it over MCP. Don't `unwrap`/panic: panics are caught, but give worse messages.
- Node functions are sync. Async work (like talking to Blender) has to be spawned with `tauri::async_runtime::spawn`. Blocking on it inside the node would panic the runtime.

### 3. UI component (`src/nodes/<id>.tsx`)

The file name must be the node id: `NodeTypes.tsx` picks up every file in `src/nodes/` by name. Most nodes are just a `BaseNode`:

```tsx
function scene_writer({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const nodeData = useNodeSpec("scene_writer");

    return <BaseNode nodeData={nodeData} inject={{}} hidden={{}} data={data} />;
}

export default scene_writer;
```

Use `inject` to put a UI element on a handle (keyed by handle id), and `hidden` to hide handles. See `animation_generator.tsx` or `get_midi_file.tsx`.

### 4. Test

Add a test in `tests/executor_test.rs` that calls the function directly with `Inputs::from([("handle_id", json!(...))])`, at minimum for a missing input and for the normal case.

## Multi Inputs

An input with `"multi": true` takes any number of connections, like Blender's multi input sockets (Animation Generator's Note On and Note Off Keyframes). The run gathers them into one list in the order they were connected, and a list connected to it is flattened in, so the executor reads it as `Option<&Vec<T>>`. Connecting the same output twice changes nothing, and `graph_disconnect` can remove one connection by its `from_node` and `from_output`.

The socket is drawn as a pill that grows with its links up to three slots, more links squeeze in closer (`multiSocketStyle` in `src/utils/sockets.ts`). `TypedEdge` ends each link at its own place on it. Outputs dragged together onto a multi input all go into it, top to bottom.

## Dynamic Handles

Dynamic handles are output handles that are generated at runtime based on the data a node receives. Unlike static handles, they are not defined in `default_nodes.json`: they are derived from the node's execution results.

### How They Work

A node can declare a `Dyn` output in `default_nodes.json`:

```json
{
    "id": "dyn_output",
    "name": "Dynamic Output",
    "data_type": "Dyn<Array<Keyframe>>"
}
```

The backend node function is then responsible for returning two things:

1. **`dyn_output`**: a map of `{ "handle_id": "Handle Name" }` which the frontend and `graph_outline` read to know what handles to show and what to call them.
2. **Flat top-level keys**: each handle's data exposed directly at the top level of the output (e.g. `"location[2]": FCurveData`). These are what the graph executor uses to pass data to downstream nodes.

```json
{
    "dyn_output": {
        "location[2]": "Location Z",
        "pose.bones[\"Arm\"].location[0]": "Arm › Location X"
    },
    "location[2]": FCurveData,
    "pose.bones[\"Arm\"].location[0]": FCurveData
}
```

Keyframes from Object only outputs the channels picked on the node (its hidden `channels` input, Blender paths with their index). It also returns every channel the object has in the hidden `available_channels` output, which its dropdowns list.

### Why Flat Top-Level Keys Are Required

The graph executor resolves connections between nodes by looking up `results[node_id][targetHandle]`. This means when a downstream node is connected to `location[2]`, the executor expects `location[2]` to exist as a direct key in the upstream node's results: not nested inside `dyn_output`. Without the flat keys, the data will never reach downstream nodes.

### Frontend

The frontend builds the handle list from the picked channels and names them from `available_channels`, see `src/nodes/keyframes_from_object.tsx`. When a channel's dropdown changes, its connections move to the new channel.

The `id` of each handle must exactly match the flat top-level key in the backend output, as this is what the executor uses to wire nodes together.
