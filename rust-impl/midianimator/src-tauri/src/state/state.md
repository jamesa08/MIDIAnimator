# Reading and Updating State

## Overview

This document explains how to use the state system in Rust and TypeScript, specifically how to read from and write to the state, and the importance of managing state access to prevent deadlocks.

## Backend: Rust module `structures::state`

To use the state system, you must import the `STATE` variable from the `structures::state` module.

### Importing the State

```rs
use MIDIAnimator::structures::state::STATE;

// writing:
use MIDIAnimator::structures::state::{STATE, update_state};
```

### Reading from the State

To read from the state, you need to lock the static `STATE` variable and unwrap it to get access. Once you have finished reading, you must `drop()` it to release the lock.

#### Example

```rs
let state = STATE.lock().unwrap();
// Read from the state as needed
let connected_application = state.connected_application.clone();
drop(state); // Release the lock to prevent deadlocks
```

### Writing to the State

To write to the state, the process is similar to reading, but you must make the state mutable. After writing to the state, `drop()` the state to release the lock and call `update_state()` to apply the changes.

#### Example

```rs
let mut state = STATE.lock().unwrap();
state.connected_application = "blender".to_string();
state.connected_version = version;
state.connected_file_name = file_name;
drop(state); // Release the lock to prevent deadlocks
update_state();  // MUST call update_state() to keep front & backend state synchronized
```

## Tabs

Every tab is its own `.mkproj` file, like an instance of the app of its own (`AppState::instances`, an `InstanceState`
each): it has its own file path and name, node graph, scene data, results, undo history, execution state and window
layout (the frontend's panels and floating windows, saved with the file). The app's parts (the Blender connection, the
node specs) stay on `AppState`. `state.active()` is the tab on screen, `state.instance(id)` any tab, and `connected_instance_id` the
tab Blender is linked to (only it gets Blender's scene changes and writes to Blender, see `go_live`).

```rs
let mut state = STATE.lock().unwrap();
state.active_mut().execution_paused = false;
drop(state);
update_state();
```

The front end gets a `StateView`: the app's parts, the `tabs` and `active_tab`, and the tab on screen's parts under
the names they had before tabs (`rf_instance`, `executed_results`, ..., and `layout`). In the frontend `frontEndState`
is the window layout of the tab on screen (`src/contexts/StateContext.tsx`).

## Why Drop the State?

Dropping the state is crucial to prevent deadlocks. A deadlock can occur when one part of the application is accessing the state while another part is trying to acquire the state lock. By dropping the state after reading or writing, you release the lock, allowing other parts of the application to access the state without getting stuck in a deadlock. `clone()` parts of the state if you need multiple parts of the application to access state. _One at a time, please!_

## Frontend: TypeScript /contexts/StateContext.tsx

Reading state from the front end is quite simple. The entire `<App>` component is wrapped in a `<StateContextProvider>`, which provides global state across the entire application.

### Importing the State

To read the state, you must import the `useStateContext` hook from `/contexts/StateContext/`. In your functional component, you must destructure the items in `useStateContext()`.

#### Example

```tsx
import { useStateContext } from "../contexts/StateContext";

function MyCustomComponent() {
    const { backEndState, setBackEndState } = useStateContext();
    return <p>{backEndState}</p>;
}
```

### Writing to the State

The front end never writes the backend state directly. A tab's node graph (`rf_instance`) is changed with ops, every one
is an undo step in that tab's history (see `graph/ops.rs`, `graph_apply` in `state/graph.rs` and `src/utils/graphOps.ts`).
The ops go to the tab in `TabContext`, which `NodeGraph` sets for the graph it shows:

```tsx
import { useGraphOps, useSetInputs } from "../utils/graphOps";

// in the node editor, `scope` is the open group's id (null at the top level)
const { apply } = useGraphOps(scope);
apply([{ op: "connect", from_node: "get_midi_file-1", from_output: "tracks", to_node: "get_midi_track_data-1", to_input: "tracks" }]);

// in a node component
const setInputs = useSetInputs();
setInputs(id, { track_name: "Piano" });
```

The backend sends the new graph back (`graph_changed`, with its `tab` and `graph_rev`, which only goes up so an older
graph never replaces a newer one, and a background tab's graph is ignored). Everything else (scene data, executed results, connection info) is owned by the backend and
changed through its own commands.

## Word of Warning
On app initalization, I am waiting for the entire App component to be rendered to send an update to the backend to retrieve the entire state object. This is the `ready` key in the state object. If your component is initalizing before the state is ready, you will get errors. You must ensure you have the updated state when `state.ready == True`. Once that is true, you are okay to modify the state.
