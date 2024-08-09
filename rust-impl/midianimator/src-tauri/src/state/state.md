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

## Why Drop the State?

Dropping the state is crucial to prevent deadlocks. A deadlock can occur when one part of the application is accessing the state while another part is trying to acquire the state lock. By dropping the state after reading or writing, you release the lock, allowing other parts of the application to access the state without getting stuck in a deadlock. `clone()` parts of the state if you need multiple parts of the application to access state. _One at a time, please!_

## Frontend: TypeScript /contexts/StateContext.tsx & `js_update_state`

Reading state from the front end is quite simple. The entire `<App>` component is wrapped in a `<StateContextProvider>`, which provides global state across the entire application.

### Importing the State

To read the state, you must import the `useStateContext` hook from `/contexts/StateContext/`. In your functional component, you must destructure the items in `useStateContext()`.

#### Example

```tsx
import { useStateContext } from "../contexts/StateContext";

function MyCustomComponent() {
    const { backendState, setBackEndState } = useStateContext();
    return <p>{backendState}</p>;
}
```

### Writing to the State

In order to write to the state, first, you must make your changes to a new object. For instance:

```tsx
const newState = { ...backendState, ready: true };
setBackendState(newState);
```

writes the backendState to a new object. This is so we can pass the updated state to the backend, where it can be processed.

In order to write to the backend, we must invoke Tauri with the custom function `js_update_state` & the single paramter `state`.

#### Example

```tsx
import { invoke } from "@tauri-apps/api/tauri";

invoke("js_update_state", {"state", JSON.stringify(newState)});
```

This will send the newly updated state to the backend, and update it. It will not send a subsequent update to the front end, so you must set the state with `setBackendState()`.

#### Full example

```tsx
import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { useStateContext } from "../contexts/StateContext";

function MyCustomComponent() {
    const { backEndState, setBackEndState } = useStateContext();

    const toggleConnected = useCallback(() => {
        let newState = { ...backEndState, connected: !backEndState.connected };
        setBackEndState(newState);
        invoke("js_update_state", { state: JSON.stringify(newState) });
    }, [backEndState]);

    return (
        <>
            <p>State object: {JSON.stringify(backEndState)}</p>
            <button onClick={toggleConnected}>Toggle Connected</button>
        </>
    );
}

export default MyCustomComponent;
```

## Word of Warning
On app initalization, I am waiting for the entire App component to be rendered to send an update to the backend to retrieve the entire state object. This is the `ready` key in the state object. If your component is initalizing before the state is ready, you will get errors. You must ensure you have the updated state when `state.ready == True`. Once that is true, you are okay to modify the state. 
