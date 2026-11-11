# MotionKeys bridge API

**Draft for review.** This document describes protocol version 1. Code references are to the code as of 2026-11-08.

The words "must", "should" and "may" have their usual spec meaning (RFC 2119).

> **Status.** The working tree implements, in `rust-impl/motionkeys/src-tauri/src/ipc/mod.rs` and `src/bridge/` (MotionKeys) and `rust-impl/blender-addon/src/` (Blender):
>
> - the transport, the envelope, `hello` and version negotiation, and one connection at a time
> - `get_scene` and `scene` events with the body of §8.2: the scene's `id`, `axes`, and each object's `transform`, `rotation_order` and channel list (`scene_generics::Scene`)
> - `watch` and its `curves` reply, the `curves` event, and Bezier keys with handles in both directions (§7.4, §8.6)
> - `write` and `write_result` (§7.5)
> - `run_script` (§7.6)
> - `info` events (§7.7)
> - the request timeouts, as MotionKeys settings (§9.5)
> - the `app` id saved in projects (§9.7)
>
> Scene objects no longer carry curves. MotionKeys keeps each object's curves itself, in its `anim_curves`. Curves saved in projects in Blender's old format are dropped on load and fetched again.
>
> MotionKeys names curves only from the bridge's channel lists (§8.5) and never parses a channel id. Its 3D preview moves its shape by semantic tags, in the object's rotation order, but doesn't read `axes` yet: it assumes z up and radians. The "Today" notes in §12 list the other differences.
>
> Not yet as specified: nothing in MotionKeys calls `run_script` yet (`ipc::run_script` exists), and the Blender bridge's change tracker is started by hand from the add-on's panel.

---

## 1. Overview

MotionKeys turns MIDI into keyframes. It doesn't talk to any 3D app directly. A **bridge** runs inside the 3D app (as an add-on, plug-in or script), connects to MotionKeys over a local TCP socket, and translates between the 3D app and this API.

In this document, **MotionKeys** is the MotionKeys application and **the 3D app** is the program the bridge runs in (Blender, Maya, Unreal Editor and so on).

A bridge is self-contained. Everything app-specific lives in it:

- its own code to read the scene, read curves and write keys, using the 3D app's scripting API
- its display name and icon
- the list of what it supports (its capabilities)

MotionKeys ships nothing app-specific and never sends code, except through the optional `run_script` message, which a bridge only receives for the script languages it offers. Anyone can write a bridge for any 3D app, including apps the MotionKeys project never planned to support, in any language that can open a TCP socket and read and write JSON. The Blender add-on (`rust-impl/blender-addon`) is the first bridge and the reference implementation.

What crosses the socket:

| Data | Direction | What it is |
|---|---|---|
| Scene | bridge → MotionKeys | The scene the user is working in: its objects, named groups of objects, and each object's channels (animatable properties). |
| Curves | bridge → MotionKeys | The keys of the objects MotionKeys reads animation from, as Bezier curves with explicit handles. |
| Writes | MotionKeys → bridge | Keys MotionKeys computed, in the same curve format. |
| Scripts (optional) | MotionKeys → bridge | Source code in a language the bridge accepts. |

Times are always in seconds. Values are in the 3D app's own units. The ids of scenes, groups, objects and channels belong to the bridge: MotionKeys stores them, saves them in projects and sends them back, but never parses them.

---

## 2. Transport and framing

- **TCP over IPv4, on 127.0.0.1 only.** MotionKeys listens and the bridge connects. Bridges must connect to `127.0.0.1`, not `localhost`, which can resolve to the IPv6 address `::1`, where MotionKeys doesn't listen.
- **The port is configurable.** MotionKeys reads it from its settings (`ipc.port`, default 6577, range 1024-65535). A bridge must let the user set the same port and should default to 6577. There is no discovery.
- **Newline-delimited JSON.** Each message is one JSON object encoded as UTF-8, followed by a newline (`\n`, byte 0x0A). The same framing is used in both directions.
  - Don't pretty-print. JSON encoders escape newlines inside strings, so a raw newline always ends a message.
  - Don't send a byte order mark.
  - Receivers ignore empty lines and strip a trailing `\r`.
- **No length limit.** Curves and writes can be many megabytes. Read into a growable buffer, and keep sending until every byte of a message is out (a non-blocking socket can accept only part of it).
- **Numbers** are JSON numbers, read as IEEE 754 doubles. NaN and infinity aren't valid JSON and must not be sent. In Python, use `json.dumps(..., allow_nan=False)`.
- **Compact output.** Write each number in its shortest round-trip form and use compact separators (Python: `separators=(",", ":")`). A sender may round values to the precision its 3D app stores (Blender stores 32-bit floats, so 9 significant digits are enough).
- **Bad lines.** A line that isn't valid JSON, or isn't an object, is logged and dropped. The connection stays open, except during the handshake (§4).

---

## 3. Envelope

Every message has the same envelope:

```json
{"id": "6f1c0b2e-4d3a-4f0e-9a51-0c7d2b8e1f45", "type": "get_scene", "reply_to": null, "body": {}}
```

| Field | Type | Meaning |
|---|---|---|
| `id` | string, required | Unique among the messages this side has sent on this connection. Any string works: a UUID (both sides use one today) or a counter. Each side numbers its own messages. |
| `type` | string, required | The message type (§7). |
| `reply_to` | string or null, required | For a reply, the `id` of the request it answers. `null` for requests and events. |
| `body` | object, required | Type-specific content. `{}` when there's nothing to send. |

**The direction tells what a message is:**

| Sent by | `reply_to` | The message is |
|---|---|---|
| MotionKeys | `null` | a request. The bridge answers it with exactly one reply. |
| MotionKeys | set | a reply: the answer to the bridge's `hello`. |
| The bridge, first message | `null` | its `hello` request. |
| The bridge, after its `hello` | set | a reply to a MotionKeys request. |
| The bridge, after its `hello` | `null` | an event: unsolicited, gets no reply. |

In version 1, MotionKeys sends no events and a bridge sends no requests besides `hello`. Adding either later would need a capability.

Rules:

- Every request gets exactly one reply: the reply type listed for it in §7, or `error`.
- Replies may arrive in any order. Match them to requests by `reply_to`.
- A reply whose `reply_to` matches no pending request (for example, one that arrives after the request timed out) is dropped.
- Unknown fields, at any level, are ignored.
- A request of an unknown `type`: the bridge replies with `error`, code `unknown_type`.
- An event of an unknown `type`: MotionKeys ignores it (a newer bridge may send events it doesn't know).
- A known request whose body can't be used: reply with `error`.

**Names.** Types, fields and capabilities use lowercase `snake_case`. Names without a dot are reserved for this spec. A bridge that adds its own message types, capabilities or body fields gives them a dotted prefix it owns, such as `com.example.take_list`, so they never clash with later versions of this spec.

---

## 4. Session

1. The bridge connects.
2. The bridge sends a `hello` request (§7.2). It sends nothing else until the reply arrives.
3. MotionKeys replies with `hello`, carrying the chosen protocol version, or with `error` and then closes the connection. Either reply's `reply_to` is the id of the bridge's `hello`.
4. During the session, MotionKeys sends requests (`get_scene`, `watch`, `write`, `run_script`). The bridge answers them and sends events (`scene`, `curves`, `info`) on its own.
5. Either side closes the socket. There is no goodbye message.

Handshake rules:

- MotionKeys sends nothing before the bridge's `hello`.
- A first message that isn't a `hello`, or a `hello` MotionKeys can't read, is answered with `error`, code `invalid_message`, and the connection is closed.
- The bridge waits up to 5 s for the reply (the Blender bridge's `HELLO_TIMEOUT`).

**One connection at a time.** While a bridge is connected, MotionKeys answers the `hello` of any further connection with `error`, code `busy`, and closes that connection. The connected bridge isn't affected. The refused bridge shows the error's `message` to the user.

**State is per connection.** The watched objects (§7.4) and everything else the bridge keeps for MotionKeys are reset when the connection closes. After a reconnect, MotionKeys sends `get_scene` and `watch` again.

An example session in the target format (bridge lines marked `B:`, MotionKeys lines `M:`, short ids instead of UUIDs, bodies shortened):

```text
B: {"id":"1","type":"hello","reply_to":null,"body":{"protocol":[1],"app":"blender", ...}}
M: {"id":"a1","type":"hello","reply_to":"1","body":{"protocol":1,"app":"motionkeys","app_version":"0.5.0"}}
M: {"id":"a2","type":"get_scene","reply_to":null,"body":{}}
B: {"id":"2","type":"scene","reply_to":"a2","body":{"id":"Scene","name":"Scene","fps":24.0, ...}}
M: {"id":"a3","type":"watch","reply_to":null,"body":{"objects":["Kick"],"fetch":["Kick"],"limit":5000,"approved":[]}}
B: {"id":"3","type":"curves","reply_to":"a3","body":{"objects":{"Kick":[ ... ]},"over_limit":{},"missing_objects":[]}}
B: {"id":"4","type":"curves","reply_to":null,"body":{"objects":{"Kick":[ ... ]}}}
M: {"id":"a4","type":"write","reply_to":null,"body":{"mode":"replace","objects":{ ... }}}
B: {"id":"5","type":"write_result","reply_to":"a4","body":{"objects":12,"keys":3400,"missing_objects":[],"errors":[]}}
```

---

## 5. Versioning and negotiation

The protocol is versioned from day one, so that a future version 2 doesn't break bridges written for version 1.

### 5.1 Version numbers

- The protocol has a **major version**, a positive integer. This document is version 1.
- In `hello`, the bridge lists every major version it speaks (`"protocol": [1]`).
- MotionKeys picks the highest version in both its own list (`PROTOCOL_VERSIONS` in `ipc/mod.rs`) and the bridge's, and returns it in its reply (`"protocol": 1`). The whole session uses that version.
- If the lists share no version, MotionKeys replies with `error`, code `unsupported_version`, whose `message` names both sides' versions, and closes the connection. The bridge tells the user which side to update.

### 5.2 Compatibility rules

Within a major version, changes are **additive only**:

- new optional fields
- new message types
- new capabilities
- new values for an existing enum field, but only sent to a peer that advertised a capability for them

Anything else needs a new major version. That includes:

- removing or renaming a field, message type or capability
- changing a field's type, units or meaning
- making an optional field required
- changing a default, or the curve evaluation rules (§8.7)
- adding a behavior every bridge must implement

Receivers ignore unknown fields, unknown events and unknown capabilities, and answer unknown requests with `error`. Senders never rely on the peer understanding something it didn't advertise.

**Optional features are capabilities (§6), not version numbers.** A bridge or MotionKeys never infers a feature from a version number. Each addition to version 1 is listed in this document with the capability or optional field that signals it.

**Some parts never change across major versions,** so that any bridge and any MotionKeys can always negotiate, or fail with a readable error. No major version may remove or change their fields; new optional fields can still be added:

- the envelope (§3)
- the `hello` request and its reply (§7.2)
- the `error` body (§7.8)

When MotionKeys adopts a new major version N, it keeps supporting N−1. A bridge written for N−1 keeps working until MotionKeys drops N−1. A bridge may also list several majors in its `hello`, to keep working with an older MotionKeys.

### 5.3 Supporting two major versions in MotionKeys

MotionKeys supports two majors at once by adapting old messages at the edge into its current internal types. Nothing past the edge knows which major a bridge speaks.

- **Internal types.** The graph, the scene diff, curve sync and the writer work on MotionKeys's own internal types (scene, curves, write, result), not on wire types.
- **One adapter per supported major.** Each major has its own wire types (for example, serde structs in `ipc::v1` and `ipc::v2` modules next to today's `Envelope` and `Hello` in `ipc/mod.rs`) and conversions both ways: wire → internal for what the bridge sends, internal → wire for what MotionKeys sends.
- **Chosen once per connection.** After `hello`, the connection stores the negotiated major and routes every line through that adapter.
- **Concepts the old major lacks.** When decoding from N−1, the adapter fills in the default, or marks the feature as unavailable the way a missing capability would. When encoding to N−1, it drops optional data, or refuses the operation with a clear error that says which bridge update is needed.
- **Changed representations.** If N changes how something is encoded (say, the key format), the N−1 adapter converts both ways, without loss wherever both forms can express the value.
- **Ids pass through unchanged.** A major version bump never changes bridge-owned ids, so saved projects stay valid across majors.
- **Tests.** Keep golden message files for each supported major, and run a fake bridge for each one in CI (the conformance test the audit suggests).

A bridge can use the same pattern in reverse to support several majors itself.

---

## 6. Capabilities

In `hello`, `capabilities` is an object. Each key is a capability name. A capability that's absent isn't supported.

Bridge capabilities in version 1:

| Capability | Value | Meaning |
|---|---|---|
| `watch` | boolean, default `false` | The bridge answers `watch`, and sends the curves of watched objects when they change (§7.4). |
| `scripts` | array of strings, default `[]` | The languages the bridge runs in `run_script` (§7.6), such as `["python"]`. |

```json
{"watch": true, "scripts": ["python"]}
```

Receivers ignore capability keys they don't know. Future capabilities may have object values, so that they can carry parameters.

MotionKeys's `hello` reply carries no capabilities in version 1. If a later revision needs bridges to know what MotionKeys supports, it adds an optional `capabilities` field to the reply.

Everything else in version 1 is required of every bridge: `hello`, `get_scene`, `write` with both modes, and the error rules. Optional data (transforms, the axis convention, semantic tags) needs no capability: it's either present or not.

---

## 7. Messages

Examples in this section are pretty-printed for reading. On the wire, each message is a single line. Most examples show only the `body`.

### 7.1 Summary

| `type` | Sent as | Direction | Body | Answered with |
|---|---|---|---|---|
| `hello` | request | bridge → MotionKeys | §7.2 | `hello` or `error` |
| `hello` | reply | MotionKeys → bridge | §7.2 | |
| `get_scene` | request | MotionKeys → bridge | `{}` | `scene` |
| `scene` | reply, event | bridge → MotionKeys | Scene (§8.2) | |
| `watch` | request | MotionKeys → bridge | §7.4 | `curves` |
| `curves` | reply, event | bridge → MotionKeys | §7.4 | |
| `write` | request | MotionKeys → bridge | §7.5 | `write_result` |
| `write_result` | reply | bridge → MotionKeys | §7.5 | |
| `run_script` | request | MotionKeys → bridge | §7.6 | `script_result` |
| `script_result` | reply | bridge → MotionKeys | §7.6 | |
| `info` | event | bridge → MotionKeys | §7.7 | |
| `error` | reply, event | either way | §7.8 | |

"Sent as" follows from the direction and `reply_to` (§3).

### 7.2 `hello`

The bridge sends `hello` as soon as it connects.

| Field | Type | Required | Meaning |
|---|---|---|---|
| `protocol` | array of integers | yes | Every major protocol version the bridge speaks. |
| `app` | string | yes | A stable id for the 3D app: lowercase ASCII letters, digits, `-`, `_` and `.`, such as `blender` or `maya`. Every bridge for the same 3D app should use the same id. Projects record it (§9.7). |
| `app_name` | string | yes | The 3D app's display name, such as `Blender`. |
| `app_version` | string | should | The 3D app's version, free-form. |
| `bridge_version` | string | should | The bridge's own version, free-form. Semantic versioning is recommended. |
| `file` | string | should | The name of the file open in the 3D app, without its folder. An empty string when the file has never been saved. (Not `null`: MotionKeys reads it as a string.) |
| `icon` | string or null | no | A small image MotionKeys shows for the connected app, as a data URL (below). |
| `capabilities` | object | should | §6. Absent means no capabilities. |

**Icon.** One string holding a data URL, either `data:image/png;base64,...` or `data:image/svg+xml;base64,...` (RFC 4648 standard base64 alphabet, with padding). Limits and guidance:

- At most 64 KiB once decoded.
- Square. A PNG should be 16-512 px on each side; 128 or 256 is recommended. An SVG should have a square `viewBox`.
- A transparent background is recommended, since MotionKeys has light and dark themes. It draws the icon small (about 24 points across today).
- An SVG must not contain scripts or references to external files.

MotionKeys only ever shows the icon as an image (an `img` element) and never inserts SVG markup into its page. It leaves out an icon that isn't a PNG or SVG data URL or decodes to more than 64 KiB (`valid_icon` in `ipc/mod.rs`), and keeps the connection. It doesn't check the image's size in pixels.

```json
{
  "protocol": [1],
  "app": "blender",
  "app_name": "Blender",
  "app_version": "5.2.0",
  "bridge_version": "0.5.0",
  "file": "drums.blend",
  "icon": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA...",
  "capabilities": {"watch": true, "scripts": ["python"]}
}
```

**Reply.** When it accepts the bridge, MotionKeys replies with `hello`, whose `reply_to` is the id of the bridge's `hello`:

| Field | Type | Meaning |
|---|---|---|
| `protocol` | integer | The major version chosen for this session. |
| `app` | string | Always `motionkeys`. |
| `app_version` | string | MotionKeys's own version. |

```json
{"protocol": 1, "app": "motionkeys", "app_version": "0.5.0"}
```

Otherwise it replies with `error` (again with `reply_to` set to the `hello`'s id) and then closes the connection. For example:

```json
{"code": "unsupported_version", "message": "…"}
```

```json
{"code": "busy", "message": "…"}
```

(The `message` texts are written by the project owner and aren't part of this spec.)

### 7.3 `get_scene` and `scene`

MotionKeys sends `get_scene` with an empty body. The bridge replies with `scene`, whose body is a Scene (§8.2). The reply should include each object's `transform` if the bridge supports transforms.

The bridge also sends `scene` as an **event** when the layout of the scene changes, including when the user switches to another scene (§9.4). The body is the same: always the whole scene, never a diff.

The channel lists in this example are shortened.

```json
{
  "id": "Scene",
  "name": "Scene",
  "fps": 24.0,
  "axes": {"up": "z", "handedness": "right", "angle_unit": "radians"},
  "object_groups": [
    {
      "id": "collection:Drums",
      "name": "Drums",
      "objects": [
        {
          "id": "Kick",
          "name": "Kick",
          "rotation_order": "XYZ",
          "transform": {"position": [0.0, 0.0, 1.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0]},
          "channels": [
            {"id": "location[0]", "name": "Location", "group": "Object", "axis": "X", "semantic": "position.x", "animated": false},
            {"id": "location[1]", "name": "Location", "group": "Object", "axis": "Y", "semantic": "position.y", "animated": false},
            {"id": "location[2]", "name": "Location", "group": "Object", "axis": "Z", "semantic": "position.z", "animated": true},
            {"id": "key_blocks[\"Hit\"].value", "name": "Hit", "group": "Shape Keys", "animated": false}
          ]
        },
        {
          "id": "Snare",
          "name": "Snare",
          "channels": [
            {"id": "pose.bones[\"Stick\"].rotation_euler[0]", "name": "Rotation", "group": "Stick", "axis": "X", "animated": true},
            {"id": "pose.bones[\"Stick\"].rotation_euler[1]", "name": "Rotation", "group": "Stick", "axis": "Y", "animated": false}
          ]
        }
      ]
    },
    {
      "id": "scene",
      "name": "Scene group",
      "objects": [
        {"id": "Camera", "name": "Camera", "channels": [
          {"id": "location[0]", "name": "Location", "group": "Object", "axis": "X", "semantic": "position.x", "animated": false}
        ]}
      ]
    }
  ]
}
```

### 7.4 `watch` and `curves`

Requires the `watch` capability. MotionKeys should not send `watch` to a bridge without it, and checks this (`ipc::bridge_watches`). A bridge that gets `watch` anyway and doesn't handle it replies with `error`, code `unknown_type`.

MotionKeys sends `watch` with the objects whose curves it needs: the objects the live tab's graph reads animation from.

| Field | Type | Required | Meaning |
|---|---|---|---|
| `objects` | array of object ids | yes | The full set of objects to watch. It replaces the previous set. An empty array stops all watching. |
| `fetch` | array of object ids | yes | The objects whose curves to send in the reply now. Each one must also be in `objects`; the bridge ignores any that aren't. |
| `limit` | integer or null | no | The import limit. A fetched object with more source keys than this, and not in `approved`, isn't sent. Absent or `null` means no limit. |
| `approved` | array of object ids | no | Objects to send even when they're over the limit. |

```json
{"objects": ["Kick", "Crowd"], "fetch": ["Kick", "Crowd"], "limit": 5000, "approved": []}
```

The bridge replies with `curves`:

| Field | Type | Meaning |
|---|---|---|
| `objects` | object: object id → array of Curve | The curves of every fetched object that's within the limit or approved, one per channel that has keys. An object without animation maps to `[]`. |
| `over_limit` | object: object id → integer | Fetched objects over the limit and not approved, with their **source key count**. Their curves aren't sent. |
| `missing_objects` | array of object ids | Fetched objects that aren't in the scene. |

```json
{
  "objects": {
    "Kick": [
      {"channel": "location[2]", "keys": [
        [0.0, 0.0, "bezier", -0.1, 0.0, 0.1, 0.0],
        [0.25, 1.0, "linear", 0.15, 1.0, 0.35, 1.0],
        [0.5, 0.0, "constant"]
      ]}
    ]
  },
  "over_limit": {"Crowd": 48210},
  "missing_objects": []
}
```

The **source key count** is the number of keys the 3D app stores for the object, over all its channels, before any conversion (§8.8). Converting an eased segment can turn one key into many, so counting converted keys would trip the limit far too often.

After replying, the bridge watches every object in `objects` except those in `over_limit`. An object over the limit is watched once a later `watch` approves it. The limit is only checked when fetching: once an object is watched, its changes are always sent.

When the keys of a watched object change, the bridge sends a `curves` **event**:

| Field | Type | Meaning |
|---|---|---|
| `objects` | object: object id → array of Curve | Each changed watched object with **all** its curves. The list replaces what MotionKeys had for that object. |

```json
{"objects": {"Kick": [{"channel": "location[2]", "keys": [[0.0, 0.0, "linear"], [0.3, 1.2, "linear"], [0.5, 0.0, "linear"]]}]}}
```

Change events are debounced (§9.4).

### 7.5 `write` and `write_result`

MotionKeys sends `write` with the keys to write, grouped per object, then per channel. It sorts each curve's keys first and, of keys less than 1e-6 s apart, keeps the last, so times always strictly increase.

| Field | Type | Required | Meaning |
|---|---|---|---|
| `mode` | string | yes | `"replace"` or `"add"` (§9.2). |
| `objects` | object: object id → array of Curve | yes | The keys to write. Each Curve holds only the keys to write to that channel. |

```json
{
  "mode": "replace",
  "objects": {
    "Tom": [
      {"channel": "location[2]", "keys": [
        [1.25, 0.0, "linear"],
        [1.3, 1.0, "bezier", 1.2833333333, 1.0, 1.35, 1.0],
        [1.45, 0.0, "linear", 1.4, 0.0, 1.5, 0.0]
      ]},
      {"channel": "key_blocks[\"Hit\"].value", "keys": [[1.25, 0.0, "linear"], [1.3, 1.0, "linear"], [1.45, 0.0, "linear"]]}
    ],
    "Snare": [
      {"channel": "location[2]", "keys": [[2.0, 0.0, "linear"], [2.05, 1.0, "linear"]]}
    ]
  }
}
```

The bridge writes everything it can. A channel that fails doesn't stop the other channels or objects. It replies with `write_result`:

| Field | Type | Meaning |
|---|---|---|
| `objects` | integer | How many objects got at least one key. |
| `keys` | integer | How many keys were written, counted as the 3D app stored them. |
| `missing_objects` | array of object ids | Objects that weren't found. Nothing was written to them. |
| `errors` | array of `{object, channel, message}` | One entry per failure. `channel` is `null` for an error about the whole object. |

```json
{
  "objects": 1,
  "keys": 3,
  "missing_objects": ["Snare"],
  "errors": [{"object": "Tom", "channel": "key_blocks[\"Hit\"].value", "message": "'Tom' has no shape keys"}]
}
```

The bridge replies with `error` instead only when it couldn't attempt the write at all, for example when the body is malformed.

### 7.6 `run_script` and `script_result`

Requires the bridge's `scripts` capability to list the language. MotionKeys checks that before sending. Its security is still to be designed, apart from scripts being off by default in MotionKeys (§10).

| Field | Type | Required | Meaning |
|---|---|---|---|
| `language` | string | yes | One of the languages the bridge listed in its `scripts` capability. |
| `source` | string | yes | The source code. |
| `args` | any JSON value | no | Arguments passed to the script. An object is recommended. |

```json
{
  "language": "python",
  "source": "import bpy\n\ndef execute(args):\n    return {\"frame\": bpy.context.scene.frame_current}\n",
  "args": {}
}
```

The bridge replies with `script_result`, holding the script's result as any JSON value:

```json
{"result": {"frame": 1}}
```

or with `error`, putting the script's traceback or error output in `detail`.

MotionKeys never interprets language names or source code. How a script receives `args` and returns its result is the bridge's convention for each language, and the bridge documents it. The Blender bridge runs `python`: the script defines `execute(args)` or `execute()`, `bpy` is available without importing it, and the return value must be JSON-serializable. A request for a language the bridge doesn't run gets `error`, code `failed`.

### 7.7 `info`

An event that updates `hello` fields that can change during a session. In version 1 that's only `file`.

```json
{"file": "drums_v2.blend"}
```

The bridge sends it when the user opens another file or saves under a new name. Absent fields are unchanged. MotionKeys updates the file name it shows.

### 7.8 `error`

| Field | Type | Required | Meaning |
|---|---|---|---|
| `code` | string | yes | A machine-readable reason (table below). An unknown or missing code is treated like `failed`. |
| `message` | string | yes | One short sentence for the user. |
| `detail` | string or null | no | More detail for logs, such as a traceback. MotionKeys shows it under the message. |

| Code | Sent by | Meaning |
|---|---|---|
| `unsupported_version` | MotionKeys | `hello`: no major version in common. |
| `busy` | MotionKeys | `hello`: another bridge is already connected. |
| `invalid_message` | MotionKeys | A `hello` MotionKeys can't read, or a first message that isn't a `hello`. |
| `unknown_type` | bridge | The request type is unknown to the bridge. |
| `failed` | bridge | The request's handler raised an error. `detail` holds the traceback. |

`error` is usually a reply. A bridge may also send it as an event to report a problem that isn't tied to a request, such as its change tracking failing. MotionKeys logs it.

---

## 8. Data types

### 8.1 Ids and names

Scenes, object groups, objects and channels each have an `id` and a `name`.

- **Ids belong to the bridge.** They're non-empty strings, compared exactly (code point by code point, case-sensitive). MotionKeys never parses, trims or rewrites them. It saves them in projects and sends them back. The one id MotionKeys builds itself is a custom property's (§8.5), from a name the user types.
- **Ids are stable.** The same thing gets the same id across reconnects, file reloads and bridge restarts. Saved projects depend on this, so a bridge must not change its id scheme once projects use it.
- **Ids are unique** within their scope: group ids and object ids within the scene, channel ids within their object.
- **Names are for display.** They may change at any time and needn't be unique.

### 8.2 Scene

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | string | yes | The scene's id. |
| `name` | string | yes | Display name. |
| `fps` | number | yes | The 3D app's real frame rate, greater than 0, such as 29.97 for NTSC (in Blender, `fps / fps_base`). |
| `axes` | Axes | no | The scene's axis convention, for the 3D preview. |
| `object_groups` | array of Object group | yes | Named sets of objects, holding every object the bridge offers (§8.3). |

The bridge sends **only the scene the user is working in**, never every scene in the file.

Times on the wire are always seconds, so `fps` never converts them. MotionKeys uses `fps` to show times as frames, for the frame variables in its mapper expressions, and to snap its playhead.

**Axes.** All fields are optional:

| Field | Values | Meaning |
|---|---|---|
| `up` | `"x"`, `"y"` or `"z"` | The up axis. |
| `handedness` | `"right"` or `"left"` | The coordinate system's handedness. |
| `angle_unit` | `"radians"` or `"degrees"` | The unit of rotation values, both in transforms and in channels tagged `rotation.*`. |

Without `axes`, the 3D preview assumes z up, right-handed and radians.

### 8.3 Object group

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | string | yes | The group's id (§8.1). |
| `name` | string | yes | Display name. |
| `objects` | array of Object | yes | The objects in the group (§8.4). |

An object group is a named set of objects. What counts as a group is the bridge's decision, such as a collection, a layer, a set or a folder. Objects only appear inside groups, so a bridge puts every object it offers into at least one group.

- **Flat.** If the 3D app nests groups, the bridge flattens them: a group contains the objects of its nested groups too.
- **May overlap.** One object can be in several groups. It then appears in full in each of them, with the same `id` and the same content. MotionKeys treats objects with the same id as one object.
- The order is the 3D app's order. MotionKeys may sort objects for its own purposes.

**Saved group references.** The Keyframes From Object and Assign Notes nodes save a group's id, in their `object_group_name` input (the input keeps its old name). MotionKeys matches a saved reference by id first, then by name, for references saved before groups had ids and for names typed by hand. A scene saved in a project before groups had ids gets each group's name as its id.

**Saved object references.** Keyframes From Object saves the object's id, in its `object_name` input, and Assign Notes keys its object map and note map by object id. MotionKeys matches a saved object reference by id first, then by name (names typed by hand), and turns a name into the id when the node is shown. Objects are always shown by `name`.

### 8.4 Object

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | string | yes | The object's id. |
| `name` | string | yes | Display name. |
| `channels` | array of Channel | yes | §8.5. May be empty. |
| `transform` | Transform | no | A snapshot of the object's transform. |
| `rotation_order` | string | no | The Euler order of the object's rotation: `XYZ`, `XZY`, `YXZ`, `YZX`, `ZXY` or `ZYX`. `XYZ` means rotate about X first, then Y, then Z (Blender's and Maya's naming). Default `XYZ`. |

**Transform:**

| Field | Type | Meaning |
|---|---|---|
| `position` | [x, y, z] | Local position (relative to the parent), in the 3D app's length unit. |
| `rotation` | [x, y, z] | Local Euler rotation in `axes.angle_unit`, applied in `rotation_order`. An object whose 3D app rotation isn't Euler (a quaternion, say) is converted. |
| `scale` | [x, y, z] | Local scale. |

The transform is for display only. It's sent when the scene is requested (`get_scene`) and may also appear in `scene` events, but it can be stale. **Bridges must not watch transforms**: a transform change on its own never causes a `scene` event.

### 8.5 Channel

A channel is one animatable value on an object.

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | string | yes | The channel's id. Opaque to MotionKeys (§8.1). |
| `name` | string | yes | Display name within its group, without the axis, such as `Location`. |
| `group` | string | yes | The label the channel is listed under, such as `Object`, a bone's name or `Shape Keys`. (Not related to the scene's groups.) |
| `axis` | string | no | `"X"`, `"Y"`, `"Z"` or `"W"` for a channel that's one component of a vector. MotionKeys uses it for axis colors. |
| `semantic` | string | no | A tag that says what the channel moves, for the 3D preview (below). |
| `animated` | boolean | no | `true` when the channel has keys now. Default `false`. |

**Channel ids are opaque.** MotionKeys stores them, saves them in projects and sends them back in writes, but never parses them. Labels, groups and axes come from the bridge, not from the id.

**Labels.** MotionKeys shows `name` followed by `axis`, such as `Location Z` for `{"name": "Location", "axis": "Z"}`, and `name` alone when there's no axis. Where a label stands alone, such as an output's name or a curve in the graph window, it puts the `group` in front, as in `Arm › Location X`, except for channels in the group of the object's **first** channel, which MotionKeys takes as the object's own. So a bridge lists the object's own channels first. A channel id that isn't in the object's list is shown as the id itself.

**Which channels to list.** The list is what MotionKeys can write to. A bridge lists:

- every animated channel of the object (one with at least one key), and
- the standard channels a user would want to write to before they're animated: the transform (position, rotation, scale), the transform of each bone of an armature, and shape keys (blend shapes, morph targets).

It may list more. The list is what MotionKeys offers in its channel pickers. It's a menu, not an allowlist: MotionKeys may write to a channel id that isn't listed (for example one the user typed in), and the bridge either writes it or reports an error for that channel.

Listing a channel doesn't mean its curve is sent. Curves are only sent for channels that have keys (§7.4).

**Shape keys are ordinary channels**, usually listed under their own `group` label.

**Custom properties have one id in every bridge.** A value the user adds to an object (Blender's custom properties, Maya's extra attributes, Unreal's user properties) has the id `["name"]`, with each `\` and `"` in the name escaped as `\\` and `\"`: `["wear"]`, `["say \"hi\""]`. One component of an array property is `["name"][i]`. The form is reserved, so a bridge never uses an id that starts with `["` for anything else, and it translates the id to its app's own path: the Blender bridge's paths are the same, a Maya bridge maps `["wiggle"]` to the `wiggle` attribute. This is what lets MotionKeys build the id from a name the user types (Static Property's Custom Property option), without knowing any app's syntax.

- A bridge lists its custom properties in this form, under a `group` of its choice (the Blender bridge's is `Custom Properties`).
- Writing to a custom property the object doesn't have creates it as a single number, 0.0 until the keys are written. A bridge whose app can't add one reports an error for that channel. Array properties aren't created, since their length isn't known.
- The form is for the object's own custom properties. Ones on a part of an object, such as a bone's, use the bridge's own ids (`pose.bones["Arm"]["glow"]` in Blender).

**Semantic tags** in version 1:

| Tag | Meaning |
|---|---|
| `position.x`, `position.y`, `position.z` | Local position along the axis, in the 3D app's length unit. |
| `rotation.x`, `rotation.y`, `rotation.z` | Local Euler rotation about the axis, in `axes.angle_unit`, applied in the object's `rotation_order`. |
| `quaternion.w`, `quaternion.x`, `quaternion.y`, `quaternion.z` | One component of the local rotation as a quaternion. |
| `scale.x`, `scale.y`, `scale.z` | Local scale along the axis. |

Tag only the object's own transform channels, not bones or delta transforms. Use each tag at most once per object. MotionKeys ignores unknown tags, so more can be added later.

### 8.6 Curve and key

A **Curve** is one channel's keys:

| Field | Type | Meaning |
|---|---|---|
| `channel` | string | The channel id. |
| `keys` | array of Key | The keys, in time order. May be empty. |

A **Key** is a JSON array in one of two forms:

```text
[time, value, interpolation]
[time, value, interpolation, left_time, left_value, right_time, right_value]
```

| Element | Type | Meaning |
|---|---|---|
| `time` | number | Seconds. Negative times are allowed. |
| `value` | number | The channel's value, in the 3D app's units. |
| `interpolation` | string | `"bezier"`, `"linear"` or `"constant"`: how the curve gets from this key to the next one. |
| `left_time`, `left_value` | numbers | The left handle, as an absolute point in seconds and value units. |
| `right_time`, `right_value` | numbers | The right handle, likewise. |

Handles are absolute positions, like Blender's `handle_left` and `handle_right` (converted to seconds), not offsets from the key.

**When handles are required.** A key must use the 7-element form if its own interpolation is `bezier`, or if the previous key's interpolation is `bezier`, because a Bezier segment uses the right handle of the key where it starts and the left handle of the key where it ends. Any other key may use the 3-element form. The 7-element form is always allowed; handles no segment uses are ignored.

**Rules:**

- Keys are sorted by strictly increasing time. No two keys share a time. A bridge whose 3D app holds several keys at one time sends one of them.
- Every number is finite.
- A left handle's time should be at or before its key's time, and a right handle's at or after. A receiver corrects a handle on the wrong side (§8.7, step 1) rather than rejecting it.
- An invalid curve (unsorted or repeated times, a non-finite number, the wrong array length, an unknown interpolation, or a missing required handle) is rejected on its own: MotionKeys ignores that curve and logs it (`bridge::curves::read_curves`), and a bridge skips that channel in a write and reports it in `errors`.

Keys are arrays rather than objects because curves can have thousands of keys, and repeating field names on every key would roughly double the size.

**Other tangent forms convert exactly:**

- **Slope tangents** (Hermite curves; tangents in Unity and Unreal without weights): put the handles one third of the segment's duration away from their keys. For a segment from key i to key i+1 lasting D seconds, with an outgoing slope m_out at key i and an incoming slope m_in at key i+1 (both in value units per second):
  - right handle of key i = (t_i + D/3, v_i + m_out·D/3)
  - left handle of key i+1 = (t_i+1 − D/3, v_i+1 − m_in·D/3)

  Time then runs linearly along the segment, and the result is exactly the Hermite curve.
- **Weighted tangents** (Maya, Unreal with weights) are already cubic Bezier control points. Convert their units to seconds and value units.
- A Bezier segment whose handles sit one third and two thirds of the way along the straight line between its keys is a straight line.

### 8.7 Curve evaluation

This section is normative. MotionKeys and every bridge evaluate curves this way, so they all compute the same curve.

Given keys k_0 … k_n−1, with times t_i, values v_i and interpolations, the value at time t is:

1. **No keys:** the curve has no value.
2. **Before the first key** (t ≤ t_0): v_0. **After the last key** (t ≥ t_n−1): v_n−1. The curve holds its end values. The format has no extrapolation.
3. **Otherwise**, find i with t_i ≤ t < t_i+1. The segment's shape comes from k_i's interpolation:
   - `constant`: v_i. The value jumps to v_i+1 at t_i+1.
   - `linear`: v_i + (v_i+1 − v_i) · (t − t_i) / (t_i+1 − t_i).
   - `bezier`: the steps below.

**Bezier segments.** The control points are P0 = (t_i, v_i), P1 = the right handle of k_i, P2 = the left handle of k_i+1, and P3 = (t_i+1, v_i+1). Each point has a time and a value.

- **Step 1: keep each handle on its own side of its key.** P1.time = max(P1.time, t_i) and P2.time = min(P2.time, t_i+1). The values don't change.
- **Step 2: no handle reaches further in time than the segment is long.** Let D = t_i+1 − t_i, L1 = P1.time − t_i and L2 = t_i+1 − P2.time.
  - If L1 > D, scale P1 toward P0: P1 = P0 + (P1 − P0) · D / L1.
  - If L2 > D, scale P2 toward P3: P2 = P3 + (P2 − P3) · D / L2.

  Both coordinates scale together, so each handle keeps its direction.
- **Step 3: solve for the curve parameter, then take the value.** With B(a, b, c, d; u) = (1−u)³a + 3(1−u)²u·b + 3(1−u)u²·c + u³d, find u in [0, 1] such that B(P0.time, P1.time, P2.time, P3.time; u) = t. The value is B(P0.value, P1.value, P2.value, P3.value; u).

**Why the curve stays a function of time.** After steps 1 and 2, both handle times lie inside [t_i, t_i+1]. That makes the time polynomial non-decreasing, so step 3 has exactly one solution even when the handles overlap in time (P2.time < P1.time). In detail, the derivative of the time polynomial is a quadratic with Bernstein coefficients a = P1.time − t_i, b = P2.time − P1.time and c = t_i+1 − P2.time, where 0 ≤ a, c ≤ D and a + b + c = D. If b < 0, then b² = (a + c − D)² ≤ ac for any such a and c, so the derivative never goes below zero, and it touches zero at no more than one point.

**Match with Blender.** Step 1 is the clamp in Blender's handle recalculation (`BKE_fcurve_handles_recalc`). Step 2 is `BKE_fcurve_correct_bezpart`. Step 3 is what Blender's `findzero` and `berekeny` compute. In MotionKeys, `segment_points` in `rust-impl/motionkeys/src-tauri/src/utils/curve.rs` does steps 1 and 2, and `evaluate` in the same file the rest. Blender works in 32-bit floats, so expect differences of around 1e-6 relative to the values. Measured: a curve MotionKeys wrote, evaluated by Blender 5.2 and 4.5, matched MotionKeys within 7.4e-7.

**Precision.** Use double precision and solve step 3 until the time error is at most 1e-9 s. Any solver works: bisection always converges because the time polynomial never decreases, and Newton's method is faster. When the handle times sit exactly at one third and two thirds of the segment, u = (t − t_i) / D directly.

Reference code (Python, written for clarity rather than speed):

```python
from bisect import bisect_right

def cubic(p0, p1, p2, p3, u):
    s = 1.0 - u
    return s * s * s * p0 + 3.0 * s * s * u * p1 + 3.0 * s * u * u * p2 + u * u * u * p3

def solve_u(t0, t1, t2, t3, t):
    # the time polynomial never decreases (see above), so bisection always converges
    lo, hi = 0.0, 1.0
    for _ in range(100):
        u = 0.5 * (lo + hi)
        x = cubic(t0, t1, t2, t3, u)
        if abs(x - t) <= 1e-9:
            return u
        if x < t:
            lo = u
        else:
            hi = u
    return 0.5 * (lo + hi)

def evaluate(keys, t):
    """The value of a curve at time t. keys are wire keys: [time, value, interp] or [time, value, interp, lt, lv, rt, rv]."""
    if not keys:
        return None
    if t <= keys[0][0]:
        return keys[0][1]
    if t >= keys[-1][0]:
        return keys[-1][1]
    # keys[i][0] <= t < keys[i + 1][0]
    i = bisect_right([key[0] for key in keys], t) - 1
    a, b = keys[i], keys[i + 1]
    t0, v0, interpolation = a[0], a[1], a[2]
    t3, v3 = b[0], b[1]
    if interpolation == "constant":
        return v0
    if interpolation == "linear":
        return v0 + (v3 - v0) * (t - t0) / (t3 - t0)
    # bezier: the right handle of a and the left handle of b
    t1, v1 = a[5], a[6]
    t2, v2 = b[3], b[4]
    # step 1: each handle stays on its own side of its key
    t1 = max(t1, t0)
    t2 = min(t2, t3)
    # step 2: no handle reaches further in time than the segment is long
    d = t3 - t0
    if t1 - t0 > d:
        f = d / (t1 - t0)
        t1, v1 = t0 + f * (t1 - t0), v0 + f * (v1 - v0)
    if t3 - t2 > d:
        f = d / (t3 - t2)
        t2, v2 = t3 + f * (t2 - t3), v3 + f * (v2 - v3)
    # step 3: the parameter where the time curve reaches t, then the value there
    u = solve_u(t0, t1, t2, t3, t)
    return cubic(v0, v1, v2, v3, u)
```

### 8.8 What the format can't express

The format has three interpolations (`bezier`, `linear`, `constant`) and held ends. When reading curves, a bridge converts anything else its 3D app does into those, so MotionKeys only ever sees `bezier`, `linear` and `constant` keys. The bridge does this because only it knows its 3D app's easing math.

- **The keyed range only.** The bridge converts a curve from its first key to its last. Past the ends, the curve holds its end values (§9.1). There's no extrapolation, and nothing is baked beyond the keys.
- **What needs converting.** In Blender: the easing interpolations (Sine, Quad, Cubic, Quart, Quint, Expo, Circ, Back, Bounce, Elastic) in all their easing modes, and F-curve modifiers such as Noise. Other apps have their own equivalents.
- **Only what's needed.** Segments the format expresses exactly are passed through unchanged. Only the affected segments are converted, or the whole keyed range when a modifier changes it. A modifier that changes nothing inside the keyed range (such as Blender's Cycles) needs no conversion.
- **How.** Evaluate with the 3D app's own evaluator and fit `bezier` segments to the result. That's the recommended method, and it gives the fewest keys. Adaptive sampling into `linear` keys is allowed as a fallback. Keep the original keys where they fall, so the curve still looks like the user's.
- **Tolerance.** Fixed by the bridge and documented, not negotiated. Recommended: at most 0.1% of the curve's value range over the converted span, with a small absolute floor (such as 1e-6) for flat curves. The Blender bridge uses this value. A 3D app that stores 32-bit floats may need a floor of a few float steps too, since 0.1% of a range only a few steps wide can't be met (§12, Conversion).
- **The import limit counts source keys**, before conversion (§7.4).

**Writes are exact.** When writing, the bridge writes MotionKeys's keys as given: the same times (converted to the 3D app's time without snapping to frames, unless the app requires it), values, interpolations and handles. It uses handle settings that keep the given positions as they are (in Blender, the `FREE` handle type). It doesn't add, drop or move keys, except where the 3D app's time resolution forces it, and `write_result` then counts the keys as stored. That way the 3D app shows exactly the curve MotionKeys computed.

---

## 9. Behavior rules

### 9.1 Held ends

Before its first key and after its last key, a curve holds the end value, in both directions. When a write creates a curve, the bridge sets the 3D app's extrapolation for it to hold (constant). In `add` mode, it leaves an existing curve's other settings (extrapolation, modifiers) as they are.

### 9.2 `replace` and `add`

- **`replace`:** for each object listed in the write, the bridge first removes **all** animation the 3D app holds for that object, on every channel, including channels the write doesn't mention. Then it writes the keys. This full wipe is intended: objects MotionKeys animates are disposable. Each bridge defines what "all animation" covers in its 3D app and documents it. Objects not listed are untouched. An object listed with no curves is only cleared.
- **`add`:** the keys go into the existing curves, and curves are created as needed. A written key replaces an existing key at the same time (within the 3D app's time resolution). Other existing keys stay.

MotionKeys never writes to an object it's watching for curves: it filters those objects out before sending a write.

### 9.3 One connection

MotionKeys talks to one bridge at a time. Any further connection gets `error`, code `busy`, in reply to its `hello`, and is closed (§4).

### 9.4 Change notifications and debouncing

**`scene` events** go out when the layout changes:

- the user switches to another scene, or renames it
- the frame rate changes
- objects are added, removed or renamed
- groups are added, removed or renamed, or their objects change
- an object's channels change: added, removed, renamed, or newly animated or no longer animated
- the axis convention or an object's rotation order changes

Send them promptly. Bursts may be coalesced with a short delay (0.2 s or less).

A bridge must **not** send a `scene` event for:

- transform changes ("don't watch those parameters, just send them when needed")
- key edits (those go out as `curves` events)
- selection, the current frame, or other property values

**`curves` events** go out when the keys of a watched object change. They're debounced: the bridge waits until the curves have stopped changing for 0.5 s (recommended; it's the Blender add-on's interval today), then sends one message with every changed object.

General rules:

- Compare with what was last sent, and send only real changes.
- When a `scene` event and a `curves` event are both pending, send the `scene` first.
- MotionKeys ignores curves for object ids that aren't in its current scene.
- A `watch` resets the bridge's baseline. Curves sent in its reply aren't sent again as an event unless they change afterward.

**Scene switches.** When the user switches to another scene in the 3D app, the bridge sends a `scene` event for the new scene. MotionKeys treats a scene with a different `id` (today: a different `name`) like a changed scene, through its existing review of scene changes: it pauses the live tab for review when the new scene's groups or objects differ from the tab's, and takes it without a pause when they're the same, as with a renamed scene.

### 9.5 Timeouts

| Wait | Limit |
|---|---|
| The bridge waits for the `hello` reply | 5 s (recommended; the Blender bridge's value) |
| `get_scene` | `ipc.timeouts.get_scene`, default 10 s |
| `watch` | `ipc.timeouts.watch`, default 30 s |
| `write` | `ipc.timeouts.write`, default 60 s |
| `run_script` | `ipc.timeouts.run_script`, default 30 s |

MotionKeys's limits are settings, in seconds, shown in Settings > Connection. `ipc::request(kind, body)` looks its limit up with `ipc::timeout(kind)` in `ipc/mod.rs`.

- When a request times out, MotionKeys reports an error and drops any late reply (§3).
- Bridges don't have to answer before the limit. A write that's still running when MotionKeys gives up may still land in the 3D app, and its late reply is dropped. MotionKeys's error then says the 3D app may still finish writing.
- When the connection closes, every pending request fails at once.

### 9.6 Ordering and threading

- TCP keeps messages in order in each direction. Replies and events may interleave.
- A bridge may handle requests one at a time, in the order they arrive. MotionKeys doesn't rely on requests being handled in parallel.
- Most 3D apps only allow scene access from their main thread. Read the socket on a background thread, hand the work to the main thread (Blender: `bpy.app.timers`, Maya: `maya.utils.executeInMainThreadWithResult`, Unreal: `unreal.register_slate_post_tick_callback`), and never block the main thread on the socket. The Blender bridge's receiving thread puts requests in a queue, and a `bpy.app.timers` function (`answer_requests` in `core.py`, every 0.02 s) answers them on the main thread, one at a time.
- Send each message whole. If several threads send, serialize the sends with a lock so two messages' bytes never mix.

### 9.7 What MotionKeys does with these messages

This section is informative, to help bridge authors predict what they'll receive.

- MotionKeys links the bridge to one tab at a time (the live tab). Only the live tab gets scene changes and writes keys.
- When a tab goes live (and after a reconnect), MotionKeys sends `get_scene`. If the objects or groups differ from the scene saved with the tab, the tab pauses until the user reviews the changes.
- It then sends `watch` for the objects the live tab's graph reads animation from. When no tab is live, it sends `watch` with an empty `objects` array.
- `scene` events update the live tab's scene and run its graph again. They're ignored when no tab is live.
- Writes come only from the live tab.
- MotionKeys names every curve from the channel list of the object it's read from or written to. A curve that isn't tied to an object yet, such as an animation generator's, takes its name from the first object in the scene that lists its channel.
- Keys made from notes keep their source handles. The note generator maps every key and both its handles through the time and amplitude mappers, the note's time offset and velocity scaling.
- Where notes overlap, keys an overlap mode keeps stay as they are. Keys it makes get MotionKeys's own smooth handles: level at peaks, valleys and ends, otherwise along the line between the neighbors, a third of the way to each, and never past a neighbor's value. The `add` overlap mode tilts each key's handles by the other curve's slope.
- Projects record the `app` id and `app_name` of the bridge whose scene they hold. When a tab goes live with a bridge whose `app` differs from the project's (a Blender project with Maya connected), MotionKeys pauses the tab and flags it in red in its scene review, by both apps' names, since object and channel ids don't match between apps. Projects saved before this have no app id and aren't flagged.
- MotionKeys's messages name the connected app by its `app_name`, and say "3D app" when nothing is connected.

---

## 10. Security of `run_script` (to be designed)

`run_script` lets MotionKeys run code inside the 3D app. Without further protection, any local process that connects to the bridge's port first could do the same, as with today's add-on. Authentication and permissions for this message are to be designed and aren't part of this draft. Nothing else in the API depends on `run_script`.

Decided so far, to be built once MotionKeys has a node that runs scripts (nothing calls `run_script` yet):

- Scripts are off by default in MotionKeys.
- When a project that contains scripts is opened, MotionKeys tells the user, like Microsoft Word's macro bar: the file has scripts, they're disabled, how to enable them, and what the security risk is. (The text is written by the project owner and isn't part of this spec.)

---

## 11. Writing a bridge

### 11.1 The minimum

A bridge must:

1. Let the user set the port (default 6577) and connect to `127.0.0.1` on it.
2. Read and write newline-delimited JSON with the envelope (§2, §3).
3. Send `hello`, and show the user the `message` of an `error` reply.
4. Answer `get_scene` with a `scene`: id, name, fps and `object_groups`, each group with an id, a name and its objects, and each object with an id, a name and its channels.
5. Answer `write` with a `write_result`, supporting both modes, all three interpolations and explicit handles, and converting seconds to the 3D app's time.
6. Answer unknown requests and failures with `error`, and ignore unknown events and fields.

That's enough for MotionKeys to animate objects in the 3D app.

### 11.2 Recommended

- `scene` events when the layout changes (§9.4).
- The `watch` capability: answering `watch`, the import limit, debounced `curves` events and conversion of what the format can't express (§8.8). This lets MotionKeys use existing animation as a source, as its Keyframes From Object node does.
- `info` events when the open file changes.
- `transform`, `axes`, `rotation_order` and semantic tags, for MotionKeys's 3D preview.
- An icon.
- `run_script`, once its security is designed (§10).

### 11.3 What a Python helper would handle

Most of a bridge is the same in every 3D app. A shared Python helper module (or a template to copy) would handle:

- **The socket:** connecting to 127.0.0.1, a background reader thread, reconnecting on request and closing cleanly.
- **Framing:** a growable line buffer, UTF-8 decoding, stripping `\r`, and whole-message sends under a lock, with partial sends handled.
- **The envelope:** message ids, telling requests, replies and events apart by direction and `reply_to`, matching replies to requests, `error` replies for unknown requests, and ignoring unknown events and fields.
- **The handshake:** building `hello` from a small configuration (app id, names, versions, an icon file read and checked against the limits), deriving `capabilities` from the handlers the author registered, and passing refusals (`unsupported_version`, `busy`) to a callback that shows them.
- **Dispatch:** running the author's handlers on the 3D app's main thread through a pluggable `run_on_main_thread` function, with timeouts. An exception becomes an `error` reply with the traceback in `detail`.
- **Watching:** keeping the watched set, the limit and the approved list, working out `over_limit` from the author's source key counts, debouncing change notifications and comparing with what was last sent.
- **Curves:** encoding and checking keys (both forms, sorted, finite), the reference evaluator from §8.7, and a converter that turns any `evaluate(t)` function over the keyed range into keys within a tolerance, by fitting Bezier segments, with adaptive sampling as a fallback.
- **JSON details:** `allow_nan=False` and compact separators.
- **Testing:** a fake MotionKeys that runs the handshake and sample requests against the bridge and checks the replies against this spec.

The author then writes only the app-specific parts:

- reading the scene: groups, objects, channels and fps, plus transforms and axes if available
- reading the curves of given objects, with their source key counts
- writing keys in `replace` or `add` mode and reporting the result
- calling the helper from the 3D app's change callbacks: the layout changed, an object's curves changed, the open file changed
- a way to run code on the 3D app's main thread
- optionally, running scripts

A sketch of what that could look like (no such module exists yet):

```python
from motionkeys_bridge import Bridge

bridge = Bridge(
    app="myapp",
    app_name="My App",
    app_version=myapp.version(),
    bridge_version="1.0.0",
    icon_path="icon.png",
    run_on_main_thread=myapp.call_on_ui_thread,
)

@bridge.on_get_scene
def read_scene():
    return {"id": ..., "name": ..., "fps": ..., "object_groups": [...]}

@bridge.on_read_curves
def read_curves(object_ids):
    # {object id: (source key count, [{"channel": ..., "keys": [...]}, ...])}
    ...

@bridge.on_write
def write(mode, objects):
    return {"objects": ..., "keys": ..., "missing_objects": [...], "errors": [...]}

bridge.connect(port=6577)

# from the 3D app's change callbacks
bridge.scene_changed()
bridge.curves_changed(object_id)
bridge.file_changed(file_name)
```

---

## 12. The Blender reference bridge

How the Blender add-on (`rust-impl/blender-addon`) implements each part. The add-on's code lives in `src/`: `core.py` (connection, hello, envelope), `handlers.py` (one handler per request), `tracker.py` (scene building and change tracking), `writer.py` (writes) and `animation.py` (reading F-curves). MotionKeys no longer holds any Blender scripts; its side of the connection is `src-tauri/src/ipc/mod.rs` and `src-tauri/src/bridge/`.

Rows describe the bridge once it implements §8. "Today" notes say where the working tree still differs.

| Part | Blender bridge |
|---|---|
| Packaging | The MotionKeys Bridge extension (`blender_manifest.toml`, Blender 4.2+), with `bl_info` for older versions. |
| Connection | A socket to `127.0.0.1`, at the port from the add-on's Port field (`Scene.motionkeys_port`, default 6577). It sends `hello`, waits up to 5 s for the reply, and shows a refusal's `message` in the connect operator's error. A receiving thread then reads messages (`core.py`). |
| Main thread | The receiving thread queues requests, and a `bpy.app.timers` function (`answer_requests`, every 0.02 s) answers them on Blender's main thread, one at a time. Sends are serialized with a lock. |
| `hello` | `protocol`: `[1]`. `app`: `blender`. `app_name`: `Blender`. `app_version`: `bpy.app.version_string`. `bridge_version`: the manifest's version. `file`: `bpy.path.basename(bpy.data.filepath)`, empty for an unsaved file. `icon`: `icon.png` in the add-on (the Blender logo, 512 px) as a PNG data URL. `capabilities`: `{"watch": true, "scripts": ["python"]}`. |
| Errors | An unknown request type gets `error` with code `unknown_type`. A handler that raises gets `error` with code `failed`, the exception's text as `message` and the traceback as `detail` (`handlers.answer`). |
| `info` | Sent from `bpy.app.handlers.load_post` and `save_post` handlers. |
| Scene | `bpy.context.scene`, the scene the user is working in. `id` and `name`: `scene.name`. `fps`: `scene.render.fps / scene.render.fps_base`. |
| Axes | `{"up": "z", "handedness": "right", "angle_unit": "radians"}`. |
| Object groups | One group per top-level child collection of the scene's collection, with nested collections flattened into it (`tracker.build_scene`). `id`: `collection:` followed by the collection's name. `name`: the collection's name. |
| Scene group | Objects linked directly to the scene's own collection go into one more group, last, with `id` `scene` and `name` `Scene group` (the owner's choice; `SCENE_GROUP` in `tracker.py`). It's sent when it has objects. Collection ids all start with `collection:`, so a collection named `Scene group` never clashes with it. |
| Objects | Every object in those groups, in full in each group it's in. `id` and `name`: `obj.name`, which is unique per file. |
| Transform | `obj.location`, the rotation as Euler angles in the object's rotation order (converted from `rotation_quaternion` or `rotation_axis_angle` for objects in those modes) and `obj.scale`, sent with every scene. Quaternions are converted with `to_euler("XYZ")`. Non-finite numbers are sent as 0 (`tracker.get_object_data`). |
| Rotation order | From `obj.rotation_mode` for the Euler modes, otherwise `XYZ`. |
| Channels listed | `animation.object_channels`, in this order. **Object** (always first): `location` (3), the rotation channels for the object's rotation mode (`rotation_euler` 3, `rotation_quaternion` 4 or `rotation_axis_angle` 4) and `scale` (3), for every object, then the object's other F-curves with keys. **Shape Keys**: the value of each shape key except the reference key, then the shape keys' other F-curves with keys. **Custom Properties**: the object's animated custom properties. Then **each pose bone** of an armature, its name as the group: its `location`, rotation channels for the bone's rotation mode and `scale`, animated or not, then its other F-curves with keys. F-curves without keys aren't listed unless they're standard channels. An F-curve whose path doesn't resolve (a removed bone or shape key) is still listed, raw, under Object or Shape Keys, so its curve can be read. |
| Channel ids | `data_path` for a property that isn't an array, and `data_path[i]` for one component of an array property: `location[2]`, `pose.bones["Arm"].rotation_euler[0]`, `["color"][1]`, `key_blocks["Smile"].value`, and `["glow"]` for a scalar custom property, which is also the protocol's custom property form (§8.5), so the bridge translates nothing. There's no migration: a Keyframes From Object pick saved with the old `[0]` form on a single-value property shows as missing and is picked again. On write, the bridge still accepts the old `[0]` form. |
| Channel labels | A static table in the add-on (`CHANNEL_LABELS` in `animation.py`) turns common data paths into names. With axes X, Y, Z: `location` → `Location`, `rotation_euler` → `Rotation`, `scale` → `Scale`, `delta_location` → `Delta Location`, `delta_rotation_euler` → `Delta Rotation`, `delta_scale` → `Delta Scale`. With axes W, X, Y, Z: `rotation_quaternion` → `Quaternion`, `rotation_axis_angle` → `Axis-Angle`, `delta_rotation_quaternion` → `Delta Quaternion` (so `Quaternion W`; Blender labels the axis-angle's angle W too). A shape key's value is named by the shape key, and a scalar custom property (the object's or a bone's) by its key. Bone channels get the same names, with the bone's name as `group`. `group` is otherwise `Object`, `Shape Keys` or `Custom Properties`. Anything else is sent raw: `name` is the channel id, with no `axis`. A bone named like one of those groups shares its label. |
| Semantic tags | `location[i]` → `position.*`, `rotation_euler[i]` → `rotation.*`, `rotation_quaternion[i]` → `quaternion.*` (W, X, Y, Z), `scale[i]` → `scale.*`, only for the object's own rotation mode: a `rotation_euler` curve on an object in quaternion mode doesn't move it, so it has no tag. No tags on bones, delta transforms or axis-angle. |
| Curves | Keys read with `keyframe_points.foreach_get` (`co`, `handle_left`, `handle_right`), with times and handle times divided by fps. `BEZIER` → `bezier`, `LINEAR` → `linear`, `CONSTANT` → `constant` (`animation.object_curves`). Of several keys on one frame, the last is sent. Below Blender 4.3, keys are read one at a time, since `foreach_get` on enums is broken there. |
| Conversion | Segments with any other interpolation are converted from `fcurve.evaluate(frame)`. A curve with other enabled modifiers is converted over its whole keyed range. A curve whose only enabled modifier is Cycles passes through unconverted, since Cycles changes nothing inside the keyed range. Only the keyed range is converted: past the ends the curve holds, whatever the F-curve's extrapolation. |
| Conversion method | Keys go first at the turning points, found from samples about 0.25 frames apart and refined by golden-section search. Each piece is then tried as one Bezier segment with one-sided slopes (the one-third rule, §8.6), then as a line, then as a step, and otherwise halved. The original key times are kept. Quad and Cubic easings can convert exactly, since they're polynomials of degree 3 or less (ease in-out needs an extra key at the midpoint). |
| Tolerance | 0.1% of the curve's value range over the converted span. The floor is the larger of 1e-6 and 2^-20 × the span's largest absolute value (8-16 float32 steps). Measured: Elastic ease-out over 30 frames gave 32 extra keys and Bounce 6, and MotionKeys's evaluation of a converted Elastic and Bounce curve stayed within 0.044% of the range. |
| `watch` | Fetches the requested objects' curves, reports the ones over the limit, and watches the rest (`handlers.watch`). Objects are looked up in the scene. |
| Source key count | `len(fcurve.keyframe_points)` summed over the object's and its shape keys' F-curves (`handlers.watch`). |
| Change detection | A `depsgraph_update_post` handler (`tracker.py`). The layout signature covers the scene's name and fps, its collection tree, and each object's name, parent, visibility, rotation mode and actions, each F-curve's data path and index and whether it has keys, its shape key names, its bones' names and rotation modes and, for an animated object, its and its bones' custom property names, **but not transforms**. It reads attributes only (no `path_resolve`): about 2 ms for 300 objects and a 160-bone rig. A path that stops resolving without any of these changing (a deleted modifier) is only picked up by the next layout change. Watched objects' keys are compared with `keyframes_signature`. A 0.1 s timer sends changes after 0.5 s without further changes, as one `curves` event with all the curves of each changed object. Today: the tracker is started by hand from the add-on's panel. |
| `replace` | Unassigns the object's and its shape keys' actions, removes them if nothing else uses them, and calls `animation_data_clear()` on both, which also removes NLA tracks and drivers (`writer.clean_all_keyframes`). |
| `add` | Keeps the existing keys. New keys are added, and `fcurve.update()` sorts them; MotionKeys's code assumes Blender merges keys less than 0.01 frames apart. |
| Written keys | Creates the F-curve where needed (action `{name}Action` or `{name}ShapeKeysAction`, with a slot on Blender 4.4+). Sets `co`, interpolation and, for keys that carry handles, `FREE` handle types with the given positions. Frames are seconds × fps, without snapping. Objects are looked up in `bpy.data.objects`. A scalar custom property channel that doesn't resolve yet (`["wear"]`, or a bone's `pose.bones["Arm"]["wear"]`) is created as a float set to 0.0, its name unescaped (§8.5), and then keyed (`writer.ensure_custom_property`). Any other channel whose property doesn't resolve is an error, and no F-curve is created for it. Below Blender 4.3, keys are written one at a time. |
| Handles on write | The bridge sets the `FREE` handles again after `fcurve.update()`, so it applies §8.7 step 1 itself: a handle on the wrong side of its key moves to the key. When Blender recalculates a curve, it moves zero-length `FREE` handles 0.001 frames off their key, which is harmless. |
| Write time limit | None: the write runs on the main thread while the request is answered. MotionKeys gives up after `ipc.timeouts.write` (default 60 s), and the write still finishes in Blender. |
| Feedback in Blender | A status bar message after each write (`writer.show_status`). |
| `run_script` | Language `python`. The source runs with `bpy` available without importing it, and the bridge calls its `execute(args)` or `execute()` function, whose return value must be JSON-serializable (`handlers.run_script`). Today: always offered, and nothing in MotionKeys calls it yet. |

---

## 13. Notes for Maya and Unreal

None of this needs MotionKeys to change. It shows that the API fits other apps, and where the bridge has to make choices.

| | Maya | Unreal Editor |
|---|---|---|
| Runs as | A Python plug-in or startup script, using `maya.cmds` and the OpenMaya API. | An editor Python script or plug-in, using the `unreal` module. |
| Main thread | Socket on a Python thread; work via `maya.utils.executeInMainThreadWithResult`. | Socket on a Python thread; work from `unreal.register_slate_post_tick_callback`. |
| Scene | The open scene (Maya has only one). fps from the current time unit (`mel.eval("currentTimeUnitToFPS")`). | The open level, plus the Level Sequence the bridge writes to. fps from the sequence's display rate. |
| Object id | The DAG full path, or the node UUID (`cmds.ls(obj, uuid=True)`), which survives renames and reparenting. Short names can clash, so they're only `name`. | The actor's path name or GUID. Actor labels can clash, so they're only `name`. |
| Groups | The bridge's choice: top-level groups, object sets or display layers. | The bridge's choice: Outliner folders (`actor.get_folder_path()`) or layers. |
| Channel ids | The attribute name for the object's own attributes (`translateX`). Attributes on other nodes, such as blendShape weights, can use `node.attribute` (`blendShape1.Smile`). Including the object's own name in its ids would change them on rename. | Track and channel, such as `Transform.Location.X`. |
| Custom properties | Extra attributes made with `cmds.addAttr`. `["wiggle"]` is the `wiggle` attribute, and a missing one is added with `cmds.addAttr(obj, longName="wiggle", attributeType="double", keyable=True)`. | Properties marked Expose to Cinematics, keyed on a property track. They can't be added from a script at write time, so a missing one is an error. |
| Shape keys | blendShape weights, listed under the mesh's object. | Morph targets don't map cleanly: Sequencer only keys them through Control Rig or animation curves, so a bridge may list none. |
| Curves in | Weighted tangents are Bezier control points, so they convert exactly. Unweighted tangents convert with the one-third rule (§8.6). `step` → `constant`. `stepnext`, the `plateau` and other computed tangent types are read as the handles Maya computed. Pre and post infinity isn't sent: the curve holds past its ends (§8.8). | Cubic keys (auto, user and break tangents) convert with the one-third rule, or exactly when tangents are weighted. Convert tangents to per-second slopes first. Linear and constant map directly. Pre and post infinity isn't sent: the curve holds past its ends (§8.8). |
| Writing | `MFnAnimCurve.addKeys` or `cmds.setKeyframe`, then `cmds.keyTangent` with weighted, fixed tangents set from the handles. Times via `MTime` in seconds. | Keys on the actor's binding in the target Level Sequence, with tangents and weights set through the scripting key API. Times snap to the sequence's tick resolution, which is fine-grained enough. |
| `replace` | `cmds.cutKey(clear=True)` on the object's channels and its blendShape weights. | Remove the binding's tracks, or their keys, in the target sequence. "The object's animation" means "this actor's tracks in that sequence". |
| Watching | `MAnimMessage.addAnimCurveEditedCallback` for key edits; `MDGMessage` and `MNodeMessage` callbacks for nodes added, removed and renamed. | Few Python hooks for Sequencer edits, so the bridge may have to poll watched channels on a timer. Editor delegates cover actors being added and removed. |
| Axes | Y up (`cmds.upAxis(q=True, axis=True)`), right-handed. Maya's API works in radians and its commands in UI units (degrees by default), so the bridge picks one unit for rotation values, uses it in both directions, and declares it in `angle_unit`. Per-object `rotateOrder` → `rotation_order`. | Z up, **left-handed**, degrees, centimeters. Rotators are roll, pitch and yaw, so the bridge maps them to x, y and z rotation for the semantic tags. |

---

## 14. Decisions and open questions

### Decisions (2026-11-08)

- Blender channel ids are `data_path` for a single value and `data_path[i]` for one component of an array, with no `[0]` and no migration (§12).
- Object groups have an `id`. The Blender bridge uses `collection:<name>` and `scene` (§8.1, §8.3, §12).
- Nodes save objects by id, and the object and note maps are keyed by id. Objects are shown by name (§8.3).
- The group of an object's first channel is the object's own, left out of standalone labels, so bridges list the object's own channels first (§8.5).
- The Blender bridge's channel names come from a static table, with unknown paths sent raw. Quaternion rotation is `Quaternion`, so it doesn't share a label with Euler `Rotation` (§12).
- Overlapping groups repeat their objects in full (§8.3).
- A channel's `name` leaves the axis out. MotionKeys shows `name` then `axis` (§8.5).
- The channel list is what MotionKeys can write to, so it includes every bone's transform. Curves are only sent for channels that have keys (§8.5).
- The `info` event stays, and the Blender bridge sends it on load and save (§7.7).
- Icons are PNG or SVG data URLs, at most 64 KiB decoded (§7.2).
- No extrapolation: a bridge converts only the keyed range, and the curve holds past its ends (§8.8).
- The bridge converts easings and modifiers into Bezier segments, within 0.1% of the value range. That's the Blender bridge's tolerance (§8.8).
- The import limit is only checked when fetching (§7.4).
- A scene switch pauses the live tab only when the groups or objects differ (§9.4).
- Timeouts are MotionKeys settings, and bridges don't have to answer before them (§9.5).
- Writes have no undo requirement. The 3D app's own undo is enough.
- Scripts are off by default, with a notice like Word's macro bar when a project with scripts opens. It's built once MotionKeys has a node that runs scripts (§10).
- MotionKeys supports the current and the previous major version (§5.2).
- Projects record the bridge's `app` id, and a mismatch pauses the tab (§9.7).
- Transforms are local, with Euler rotation in `rotation_order` (§8.4).

### Decisions (2026-11-11)

- A custom property has the id `["name"]` in every bridge, escaped, and the form is reserved. Each bridge translates it to its app's own path, and MotionKeys builds it from a typed name. Writing to a missing one creates it as a number (§8.5).

### Still open

- How the script notice remembers a file the user trusts.
- Whether bridges also need their own switch for `run_script`, off by default. The Blender bridge always offers `python` today.
- Later: chunked writes for very large writes (a capability), and a `bridge_id` to tell apart several bridges for one app.
