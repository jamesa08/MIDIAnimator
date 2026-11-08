# Bridge connection

The `ipc` module talks to the connected 3D app through its bridge (the Blender add-on is the first one). The full protocol is in [`docs/bridge_protocol.md`](../../../../../docs/bridge_protocol.md); this is the short version for working on the Rust side.

## Messages

A bridge connects to `127.0.0.1` on the `ipc.port` setting (6577). Both sides send one JSON object per line:

```json
{"id": "7b1e...", "type": "get_scene", "reply_to": null, "body": {}}
```

- `reply_to` is the id of the request a message answers, and `null` for a request or a message sent unasked.
- The bridge's first message is a `hello` with the protocol versions it speaks, its app's id, name and version, the open file, an icon (a data URL) and its capabilities. MotionKeys answers with a `hello` naming the version both speak (`ipc::PROTOCOL_VERSIONS`), or an `error` with a `code` and closes the connection.
- One bridge is connected at a time. A second one is turned away (`busy`).
- After the hello, MotionKeys sends requests (`get_scene`, `watch`, `write`, `run_script`) and the bridge answers each with a reply (`scene`, `curves`, `write_result`, `script_result`) or an `error`. The bridge also sends `scene` unasked when the scene changes, `curves` when the keys of a watched object change, and `info` when the open file's name changes. Curves are `{channel, keys}`, each key `[time, value, interpolation]` or with its two Bezier handles, in seconds (`utils/curve.rs`). A scene (`scene_generics::Scene`) lists each object's channels, with their names, groups, axes and semantic tags: MotionKeys names curves from these and never parses a channel id.

## Sending a request

`ipc::request` sends a request to the connected bridge and waits for the reply's body, as long as the request's `ipc.timeouts` setting says (`ipc::timeout`). An `error` reply, no bridge or a disconnect come back as `RequestError::Failed`, no reply in time as `RequestError::Timeout`, and `?` turns either into its message:

```rs
use motionkeys::ipc;
use serde_json::json;

async fn scene_name() -> Result<String, String> {
    let scene = ipc::request("get_scene", json!({})).await?;
    Ok(scene["name"].as_str().unwrap_or_default().to_string())
}
```

`ipc::run_script` runs a script in the connected app, when its bridge said in its hello that it runs that language. A Python script for the Blender bridge defines `execute(args)` (or `execute()`) and returns JSON; `bpy` is there without importing it.
