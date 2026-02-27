# Nodes and Backend

## Dynamic Handles

Dynamic handles are output handles that are generated at runtime based on the data a node receives. Unlike static handles, they are not defined in `default_nodes.json`: they are derived from the node's execution results.

### How They Work

A node can declare a `Dyn` output in `default_nodes.json`:

```json
{
    "id": "dyn_output",
    "name": "Dynamic Output",
    "type": "Dyn<Array<Keyframe>>"
}
```

The backend node function is then responsible for returning two things:

1. **`dyn_output`**: a map of `{ "handle_name": FCurveData }` which the frontend reads to know what handles to render.
2. **Flat top-level keys**: each handle's data exposed directly at the top level of the output (e.g. `"location_x": [Keyframe]`). These are what the graph executor uses to pass data to downstream nodes.

```json
{
    "dyn_output": {
        "location_x": FCurveData,
        "location_z": FCurveData
    },
    "location_x": [Keyframe],
    "location_z": [Keyframe]
}
```

### Why Flat Top-Level Keys Are Required

The graph executor resolves connections between nodes by looking up `results[node_id][targetHandle]`. This means when a downstream node is connected to `location_x`, the executor expects `location_x` to exist as a direct key in the upstream node's results: not nested inside `dyn_output`. Without the flat keys, the data will never reach downstream nodes.

### Frontend

The frontend reads `dyn_output` from `executed_results` to build the handle list:

```typescript
const animCurves = executedResults?.dyn_output
    ? Object.keys(executedResults.dyn_output).map((curveName) => ({
          id: curveName,
          name: curveName.split("_").join(" ").toProperCase(),
          type: "Array<Keyframe>",
      }))
    : [];
```

The `id` of each handle must exactly match the flat top-level key in the backend output, as this is what the executor uses to wire nodes together.
