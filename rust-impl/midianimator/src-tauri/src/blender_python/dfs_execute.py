from pprint import pprint
default_nodes = {
    "nodes": [
        {
            "id": "add_numbers",
            "name": "Add Numbers",
            "description": "add some numbers",
            "executor": "rust",
            "realtime": True,
            "handles": {
                "inputs": [
                    {
                        "id": "item1",
                        "name": "Item 1",
                        "type": "number"
                    },
                    {
                        "id": "item2",
                        "name": "Item 2",
                        "type": "number"
                    }
                ],
                "outputs": [
                    {
                        "id": "result",
                        "name": "Result",
                        "type": "number"
                    }
                ]
            }
        },
        
        {
            "id": "subtract_numbers",
            "name": "Subtract Numbers",
            "description": "subtract some numbers",
            "executor": "rust",
            "realtime": True,
            "handles": {
                "inputs": [
                    {
                        "id": "item1",
                        "name": "Item 1",
                        "type": "number"
                    },
                    {
                        "id": "item2",
                        "name": "Item 2",
                        "type": "number"
                    }
                ],
                "outputs": [
                    {
                        "id": "result",
                        "name": "Result",
                        "type": "number"
                    }
                ]
            }
        },
        
        {
            "id": "multiply_numbers",
            "name": "Multiply Numbers",
            "description": "multiply some numbers",
            "executor": "rust",
            "realtime": True,
            "handles": {
                "inputs": [
                    {
                        "id": "item1",
                        "name": "Item 1",
                        "type": "number"
                    },
                    {
                        "id": "item2",
                        "name": "Item 2",
                        "type": "number"
                    }
                ],
                "outputs": [
                    {
                        "id": "result",
                        "name": "Result",
                        "type": "number"
                    }
                ]
            }
        },
        
        {
            "id": "divide_numbers",
            "name": "Divide Numbers",
            "description": "Divide some numbers",
            "executor": "rust",
            "realtime": True,
            "handles": {
                "inputs": [
                    {
                        "id": "item1",
                        "name": "Item 1",
                        "type": "number"
                    },
                    {
                        "id": "item2",
                        "name": "Item 2",
                        "type": "number"
                    }
                ],
                "outputs": [
                    {
                        "id": "result",
                        "name": "Result",
                        "type": "number"
                    }
                ]
            }
        }
    ]
}

rf_instance = {
    "edges": [
        {
            "source": "add_numbers",
            "sourceHandle": "result",
            "target": "subtract_numbers",
            "targetHandle": "item1"
        },
        
        {
            "source": "add_numbers",
            "sourceHandle": "result",
            "target": "multiply_numbers",
            "targetHandle": "item1"
        },
        
        {
            "source": "multiply_numbers",
            "sourceHandle": "result",
            "target": "subtract_numbers",
            "targetHandle": "item2"
        },
        
        {
            "source": "divide_numbers",
            "sourceHandle": "result",
            "target": "multiply_numbers",
            "targetHandle": "item2"
        }
    ],
    "nodes": [
        { "id": "add_numbers", "data": { "inputs": { "item1": 1, "item2": 3 } } },
        { "id": "subtract_numbers", "data": { } },
        { "id": "multiply_numbers", "data": { } } ,
        { "id": "divide_numbers", "data": { "inputs": { "item1": 4, "item2": 3 } } } 
    ]
}

def execute_dfs(node_id: str, visited: set, results: dict):
    print("EXECUTING NODE:", node_id)
    if node_id in visited:
        print("ALREADY EXECUTED", node_id)
        return results
    
    node = [item for item in rf_instance["nodes"] if item["id"] == node_id][0]
    node_data = node["data"]
    result = {}
    
    incoming_edges = [edge for edge in rf_instance["edges"] if edge["target"] == node_id]
    
    result["inputs"] = dict()
    
    if "inputs" in node_data:
        print("FOUND COMPUTED INPUT DATA", node_id)
        # we have computed inputs
        result["inputs"] = node_data["inputs"]
    
    for edge in incoming_edges:
        if edge["source"] in results:
            # get node_results via looking up the source node
            node_results = results[edge["source"]]
            
            # add computed result to result lookup table
            # target handle: the "input" for the node
            # the value: the stored result
            result["inputs"][edge["targetHandle"]] = node_results[edge["sourceHandle"]]
        else:
            print(f"NOT EXECUTED YET FOR EDGE {edge}... executing on {edge['source']}")
            # not executed yet, execute first as it is a dependent
            return execute_dfs(edge["source"], visited, results)  # return early as we don't want to continue execution early

    # add execution results to results
    result = execute_node(node_id, result["inputs"])
    
    # after successful execution, we can say we visited the node
    visited.add(node_id)

    results[node_id] = result
    
    for edge in rf_instance["edges"]:
        if edge["source"] == node_id:
            execute_dfs(edge["target"], visited, results)
    
    return results
    

def execute_node(node_id: str, inputs: dict):
    if node_id == "add_numbers":
        outputs = {}

        outputs["result"] = inputs["item1"] + inputs["item2"]
        return outputs
    
    elif node_id == "subtract_numbers":
        outputs = {}

        outputs["result"] = inputs["item1"] - inputs["item2"]
        return outputs
    
    elif node_id == "multiply_numbers":
        outputs = {}

        outputs["result"] = inputs["item1"] * inputs["item2"]
        return outputs
    
    elif node_id == "divide_numbers":
        outputs = {}

        outputs["result"] = inputs["item1"] / inputs["item2"]
        return outputs


pprint(execute_dfs("add_numbers", set(), {}))