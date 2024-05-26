# IPC Python Files

## Components
### Rust Module: `ipc`

The ipc module in Rust handles the communication with Blender. Messages are exchanged in JSON format, structured with the following parameters:

- `sender`: Identifies the sender of the message.
- `message`: Contains the data or command to be executed.
- `uuid`: A unique identifier for the message.

### Message Structure
Messages sent via the `ipc` module under the hood follow this JSON format:

```json
{
  "sender": "example_sender",
  "message": "your_message_data_here",
  "uuid": "unique_message_identifier"
}
```

### Sending a Message

To send a message from Rust and receive the result, use the `ipc::send_message` function. This function is asynchronous, so it must be awaited. Here is a demonstration:


```rs
use MIDIAnimator::structures::ipc;

static PYTHON_FILE: &str = include_str!("some_python_file.py");

#[tokio::main]
async fn main() {
    let result = match ipc::send_message(PYTHON_FILE.to_string()).await {
        Some(data) => data,
        None => {
            panic!("Error executing");
        }
    };

    println!("Result from Blender: {:?}", result);
}
```

## Python Scripts

Python scripts are used to interact with Blender's Python API (bpy). These scripts are executed within Blender's environment and should follow a specific structure.

### Python Script Requirements

Python scripts should define an `execute()` function, which **must** return a value. There are no specific type requirements for the return value, but it must not be `None`. Scripts can include additional functions, classes, and module imports as needed. The `bpy` module is pre-imported, but can be re-imported if necessary.

Example Python script:


```py
import bpy

def additional_function():
    pass

def execute():
    scene = bpy.context.scene
    return scene.name

```

