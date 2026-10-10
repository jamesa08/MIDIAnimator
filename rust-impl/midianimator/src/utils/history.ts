// row text for each undo step by the op that made it (Op::name in src-tauri/src/graph/ops.rs, and the MCP tools'
// ops). placeholders until the real copy is written, an op missing here shows its own name
const LABELS: Record<string, string> = {
    add_nodes: "Add Nodes",
    add_node: "Add Node",
    delete: "Delete",
    remove_node: "Remove Node",
    connect: "Connect",
    disconnect: "Disconnect",
    set_inputs: "Set Inputs",
    set_label: "Set Label",
    move: "Move",
    resize: "Resize",
    select: "Select",
    duplicate: "Duplicate",
    paste: "Paste",
    cut: "Cut",
    group: "Group",
    ungroup: "Ungroup",
    rename_socket: "Rename Socket",
    remove_socket: "Remove Socket",
    make_local: "Make Local",
    revert_group: "Revert to Built-in",
};

// what an undo step is called
export const historyLabel = (op: string) => LABELS[op] ?? op;
