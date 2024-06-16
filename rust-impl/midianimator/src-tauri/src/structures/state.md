# Reading and Updating State

## Overview
This document explains how to use the state system in Rust, specifically how to read from and write to the state, and the importance of managing state access to prevent deadlocks.

## Rust module `structures::state`

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
update_state();
```

## Why Drop the State?
Dropping the state is crucial to prevent deadlocks. A deadlock can occur when one part of the application is accessing the state while another part is trying to acquire the state lock. By dropping the state after reading or writing, you release the lock, allowing other parts of the application to access the state without getting stuck in a deadlock. `clone()` parts of the state if you need multiple parts of the application to access state. *One at a time, please!*

