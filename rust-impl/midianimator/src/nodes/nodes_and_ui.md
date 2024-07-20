# Nodes and UI

## Overview

How nodes are managed in backend


JSON file defines basic node structure. This is shared between Rust and Typescript.

```json
{
    "nodes": [
        {
            "id": "example_node",
            "name": "Example Node",
            "description": "An example",
            "executor": "js", // OR "rust
            "handles": {
                "inputs": [
                    {
                        "id": "example_input",
                        "name": "Example input handle",
                        "type": "i32",  // still defining what this does
                        "hidden": false // show the handle in the UI? (useful for having hidden data props) aside: should this be STRICTLY in the UI file, and not in the JSON config?
                    }
                ],
                "outputs": [

                ]
            }
        }
    ]
}
```