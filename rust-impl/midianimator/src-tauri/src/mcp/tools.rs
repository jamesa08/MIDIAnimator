// MCP tool definitions. the graph logic lives in `crate::graph::{model, edit, outline}`,
// this file only reads/writes `STATE`, runs execution and formats the results

use std::collections::HashMap;
use std::sync::{MutexGuard, PoisonError};

use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::graph::edit::{self, EditResult};
use crate::graph::execute::{execute_graph, panic_message};
use crate::graph::model::{describe_type, is_param, node_specs, Graph, NodeSpec, Position};
use crate::graph::outline::{self, input_options, node_block, node_errors, Detail, OutlineCtx};
use crate::state::{load_project_from, save_project_to, update_state, AppState, STATE};

// instructions sent to the MCP client when it connects
const INSTRUCTIONS: &str = "MotionKeys: node graphs that turn MIDI into Blender keyframes. \
Call graph_outline first to see the current graph, and node_types_list for the node types you can add. \
Data flows from outputs to inputs; edit with graph_add_node, graph_connect, graph_set_inputs, graph_disconnect and graph_remove_node. \
Never connect hidden handles: 'par' inputs are set with graph_set_inputs, and outputs marked hidden are display-only. \
Edits show up live in the app and re-run the realtime graph, which never writes to Blender; \
graph_execute with write_to_blender=true writes keyframes to Blender. Node ids accept any unique prefix.";

// returned by every tool when the frontend hasn't called `ready` yet
const NOT_READY: &str = "app not ready: the MotionKeys window has not finished loading; try again in a moment";

// MARK: - Parameters
// note: the `///` comments on these fields end up in the tool schemas the ML model sees, so they're written for it

// graph_outline
#[derive(Debug, Deserialize, JsonSchema)]
pub struct OutlineParams {
    /// Only show this node and the nodes upstream and downstream of it (id or unique id prefix)
    pub scope: Option<String>,
    /// "concise" (default) or "full" (adds descriptions and the values arriving on connected inputs)
    pub detail: Option<String>,
}

// node_describe, graph_remove_node
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NodeParams {
    /// Node id or unique id prefix, e.g. "get_midi_file-1"
    pub node: String,
}

// types_describe
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TypeParams {
    /// Handle type name as shown in node_types_list, e.g. "Array<MIDINote>"
    pub data_type: String,
}

// graph_add_node
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddNodeParams {
    /// Node type id from node_types_list, e.g. "get_midi_file"
    pub node_type: String,
    /// Initial input values keyed by input id, e.g. {"file_path": "/abs/path/song.mid"}
    pub inputs: Option<Map<String, Value>>,
    /// Canvas position; omit to place automatically
    pub position: Option<Position>,
    /// Place the new node to the right of this node (id or prefix) when position is omitted
    pub after: Option<String>,
}

// graph_connect
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ConnectParams {
    /// Producing node (id or unique prefix)
    pub from_node: String,
    /// Output handle id on the producing node
    pub from_output: String,
    /// Consuming node (id or unique prefix)
    pub to_node: String,
    /// Input handle id on the consuming node; an existing connection into it is replaced
    pub to_input: String,
}

// graph_disconnect
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DisconnectParams {
    /// Consuming node (id or unique prefix)
    pub to_node: String,
    /// Input handle id whose connection is removed
    pub to_input: String,
}

// graph_set_inputs
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetInputsParams {
    /// Node id or unique id prefix
    pub node: String,
    /// Input values keyed by input id; merged into the node's values, null unsets one
    pub inputs: Map<String, Value>,
}

// graph_execute
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExecuteParams {
    /// false: realtime run (skips nodes that write to Blender). true: full run, writes keyframes to Blender
    pub write_to_blender: bool,
}

// project_load, project_save
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PathParams {
    /// Absolute path to a .mkproj project file
    pub path: String,
}

// MARK: - State Helpers

/// locks the global state, a poisoned lock (a panic somewhere else) shouldn't take the MCP server down too
fn lock_state() -> MutexGuard<'static, AppState> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// a copy of the parts of `AppState` the tools read
struct Snapshot {
    state: AppState,
    graph: Graph,
    specs: Vec<NodeSpec>,
}

impl Snapshot {
    /// clones the state and parses the graph and node specs out of it
    fn take() -> Result<Self, String> {
        // copy the state so we don't hold the lock while the tool runs
        let state = lock_state().clone();
        // don't do anything until the frontend has loaded and called `ready`
        if !state.ready {
            return Err(NOT_READY.to_string());
        }
        // parse the graph and the node specs
        let graph = Graph::from_rf(&state.rf_instance)?;
        let specs = node_specs(&state.default_nodes);
        Ok(Self {
            state,
            graph,
            specs,
        })
    }

    /// borrows the snapshot as the context the outline functions need
    fn ctx(&self) -> OutlineCtx<'_> {
        OutlineCtx {
            graph: &self.graph,
            specs: &self.specs,
            results: &self.state.executed_results,
            inputs: &self.state.executed_inputs,
            scene_data: &self.state.scene_data,
        }
    }
}

/// a successful tool result with some text
fn ok_text(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

/// a failed tool result with an error message the ML model can read
fn tool_error(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![ContentBlock::text(text)]))
}

/// runs `execute_graph` in its own task so a panicking executor turns into an error message instead
async fn run_execution(realtime: bool) -> Result<(), String> {
    tokio::spawn(execute_graph(realtime)).await.map_err(|e| {
        // pull the panic message out
        if e.is_panic() {
            format!("execution failed: {}", panic_message(&e.into_panic()))
        } else {
            format!("execution failed: {}", e)
        }
    })
}

/// runs the realtime graph unless execution is paused, returns a one-line status
async fn run_realtime_if_allowed() -> String {
    // grab what we need and drop the lock before executing
    let (paused, empty) = {
        let state = lock_state();
        (state.execution_paused, Graph::from_rf(&state.rf_instance).map_or(true, |g| g.nodes.is_empty()))
    };
    // don't run while scene changes are waiting to be reviewed in the app
    if paused {
        return "execution paused (Blender scene changes are pending review in the app); results not updated".to_string();
    }
    if empty {
        return "nothing to execute".to_string();
    }
    // run it, if it fails the last results are still in the state
    if let Err(e) = run_execution(true).await {
        return format!("{}; values below are from the last successful run", e);
    }
    // the edit worked either way, but say which nodes failed
    let errors = node_errors(&lock_state().executed_results);
    if errors.is_empty() {
        "realtime execution ok".to_string()
    } else {
        format!("realtime execution: {} node(s) failed, the nodes after them didn't run\n{}", errors.len(), errors.join("\n"))
    }
}

// MARK: - Server

/// the MCP server handler, one per session
#[derive(Clone)]
pub struct MotionKeysMcp {
    tool_router: ToolRouter<Self>,
}

impl MotionKeysMcp {
    /// applies one edit to the stored graph, pushes it to the UI, re-runs the realtime graph
    /// and reports what changed plus the updated outline of the touched nodes
    async fn apply_edit<F>(&self, edit: F) -> Result<CallToolResult, McpError>
    where
        F: FnOnce(&mut Graph, &[NodeSpec], &HashMap<String, Value>) -> Result<EditResult, String>,
    {
        // lock the state for the edit only, the lock is dropped before execution
        let result = {
            let mut state = lock_state();
            if !state.ready {
                return tool_error(NOT_READY);
            }
            // parse the graph out of the state
            let mut graph = match Graph::from_rf(&state.rf_instance) {
                Ok(graph) => graph,
                Err(e) => return tool_error(e),
            };
            // apply the edit, only write the graph back to the state if it worked
            let specs = node_specs(&state.default_nodes);
            match edit(&mut graph, &specs, &state.executed_results) {
                Ok(result) => {
                    state.rf_instance = graph.to_rf();
                    result
                }
                Err(e) => return tool_error(e),
            }
        };

        // push the new graph to the UI, then re-run the realtime graph
        update_state();
        let execution = run_realtime_if_allowed().await;
        self.edit_report(&result, &execution)
    }

    /// builds the tool result for an edit: the message, the execution status and the outline of each touched node
    fn edit_report(&self, result: &EditResult, execution: &str) -> Result<CallToolResult, McpError> {
        // take a fresh snapshot so the outline shows the new results
        let snapshot = match Snapshot::take() {
            Ok(snapshot) => snapshot,
            Err(e) => return tool_error(e),
        };
        let ctx = snapshot.ctx();
        // message and execution status first, then one block per touched node
        let mut text = format!("{}\n{}", result.message, execution);
        for id in &result.touched {
            text.push_str("\n\n");
            text.push_str(&node_block(&ctx, id, Detail::Concise));
        }
        ok_text(text)
    }
}

#[tool_router]
impl MotionKeysMcp {
    /// creates the server with all the `#[tool]` functions below registered
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    // MARK: - Read-only Tools

    // app_status: ready flag, blender connection, graph size and execution state
    #[tool(description = "App status: whether the UI is ready, Blender connection (app, version, file), node and edge counts, whether execution is paused and scene changes are pending.", annotations(read_only_hint = true))]
    async fn app_status(&self) -> Result<CallToolResult, McpError> {
        let state = lock_state().clone();
        // count the nodes and edges, 0 if the graph can't be read
        let (nodes, edges) = Graph::from_rf(&state.rf_instance).map_or((0, 0), |g| (g.nodes.len(), g.edges.len()));
        // describe the blender connection
        let blender = if state.connected {
            format!(
                "connected to {} {} ({})",
                state.connected_application,
                state.connected_version,
                if state.connected_file_name.is_empty() {
                    "unsaved file"
                } else {
                    &state.connected_file_name
                }
            )
        } else {
            "not connected".to_string()
        };
        ok_text(format!("ready: {}\nblender: {}\ngraph: {} nodes, {} edges\nexecution paused: {}\npending scene changes: {}", state.ready, blender, nodes, edges, state.execution_paused, state.pending_scene_data.is_some()))
    }

    // node_types_list: every node type with its inputs and outputs
    #[tool(description = "List the node types that can be added, with their inputs and outputs (id, UI name, type, description). 'par' inputs are hidden in the UI: set them with graph_set_inputs, never connect them. Outputs marked hidden are display-only and must not be connected either.", annotations(read_only_hint = true))]
    async fn node_types_list(&self) -> Result<CallToolResult, McpError> {
        let specs = node_specs(&lock_state().default_nodes);
        // one block per node type
        let blocks: Vec<String> = specs
            .iter()
            .map(|spec| {
                // header line: id, name, description and whether it's realtime
                let mut lines = vec![format!(
                    "{}  \"{}\"  {}{}",
                    spec.id,
                    spec.name,
                    spec.description,
                    if spec.realtime {
                        ""
                    } else {
                        "  [not realtime: runs only with graph_execute write_to_blender=true]"
                    }
                )];
                // one line per input, parameters (hidden inputs) are marked `par`
                for input in &spec.handles.inputs {
                    let kind = if is_param(spec, &input.id) {
                        "par"
                    } else {
                        "in "
                    };
                    lines.push(format!("  {}  {} \"{}\": {} — {}", kind, input.id, input.name, input.data_type, input.description));
                }
                // one line per output, hidden ones are marked so they don't get connected
                for output in &spec.handles.outputs {
                    let hidden = if output.hidden {
                        " [hidden, do not connect]"
                    } else {
                        ""
                    };
                    lines.push(format!("  out  {} \"{}\": {}{} — {}", output.id, output.name, output.data_type, hidden, output.description));
                }
                lines.join("\n")
            })
            .collect();
        ok_text(blocks.join("\n\n"))
    }

    // graph_outline: outline of the whole graph (or part of it with `scope`)
    #[tool(description = "Text outline of the node graph, producers first. One block per node: inputs ('<-' source), parameters (= value, options), outputs ('->' targets) with a summary of the last executed value in [brackets].", annotations(read_only_hint = true))]
    async fn graph_outline(&self, Parameters(params): Parameters<OutlineParams>) -> Result<CallToolResult, McpError> {
        let snapshot = match Snapshot::take() {
            Ok(snapshot) => snapshot,
            Err(e) => return tool_error(e),
        };
        // parse the detail level, default is concise
        let detail = match params.detail.as_deref() {
            None | Some("concise") => Detail::Concise,
            Some("full") => Detail::Full,
            Some(other) => return tool_error(format!("unknown detail '{}'; use \"concise\" or \"full\"", other)),
        };
        // add the node and edge counts on top of the outline
        match outline::outline(&snapshot.ctx(), params.scope.as_deref(), detail) {
            Ok(text) => ok_text(format!("{} nodes, {} edges\n\n{}", snapshot.graph.nodes.len(), snapshot.graph.edges.len(), text)),
            Err(e) => tool_error(e),
        }
    }

    // node_describe: full detail block for one node
    #[tool(description = "Describe one node in full: every input with its value or connection and valid options, every output with targets and a summary of its value.", annotations(read_only_hint = true))]
    async fn node_describe(&self, Parameters(params): Parameters<NodeParams>) -> Result<CallToolResult, McpError> {
        let snapshot = match Snapshot::take() {
            Ok(snapshot) => snapshot,
            Err(e) => return tool_error(e),
        };
        // resolve the id (prefixes are allowed) and show it in full detail
        match snapshot.graph.resolve(&params.node) {
            Ok(id) => ok_text(node_block(&snapshot.ctx(), &id, Detail::Full)),
            Err(e) => tool_error(e),
        }
    }

    // types_describe: JSON schema for a handle type
    #[tool(description = "JSON schema of a handle data type such as Array<MIDINote>, MIDITrack, ObjectGroup or Array<Keyframe>.", annotations(read_only_hint = true))]
    async fn types_describe(&self, Parameters(params): Parameters<TypeParams>) -> Result<CallToolResult, McpError> {
        match describe_type(&params.data_type) {
            Ok(text) => ok_text(text),
            Err(e) => tool_error(e),
        }
    }

    // MARK: - Write Tools

    // graph_add_node
    #[tool(description = "Add a node. Returns its new id (e.g. get_midi_file-2). Without position it is placed right of 'after', or right of the right-most node.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_add_node(&self, Parameters(params): Parameters<AddNodeParams>) -> Result<CallToolResult, McpError> {
        // add the node through apply_edit so the UI and realtime results get updated
        self.apply_edit(|graph, specs, _| edit::add_node(graph, specs, &params.node_type, params.inputs.as_ref(), params.position, params.after.as_deref())).await
    }

    // graph_connect
    #[tool(description = "Connect an output to an input (data flows from_node.from_output -> to_node.to_input). Checks types and cycles; replaces any existing connection into that input. Hidden handles ('par' inputs and outputs marked hidden) must never be connected and are refused.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_connect(&self, Parameters(params): Parameters<ConnectParams>) -> Result<CallToolResult, McpError> {
        self.apply_edit(|graph, specs, results| edit::connect(graph, specs, results, &params.from_node, &params.from_output, &params.to_node, &params.to_input)).await
    }

    // graph_disconnect
    #[tool(description = "Remove the connection into one input.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_disconnect(&self, Parameters(params): Parameters<DisconnectParams>) -> Result<CallToolResult, McpError> {
        self.apply_edit(|graph, _, _| edit::disconnect(graph, &params.to_node, &params.to_input)).await
    }

    // graph_set_inputs
    #[tool(description = "Set input values on a node (parameters like file_path or track_name, or unconnected inputs). Keys must be declared input ids; values are merged, null unsets.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_set_inputs(&self, Parameters(params): Parameters<SetInputsParams>) -> Result<CallToolResult, McpError> {
        // warn about values that aren't one of the currently known options
        let mut warnings: Vec<String> = Vec::new();
        if let Ok(snapshot) = Snapshot::take() {
            if let Some(node) = snapshot.graph.resolve(&params.node).ok().and_then(|id| snapshot.graph.node(&id)) {
                // compare each string value against the options for that input (if we know them)
                for (key, value) in &params.inputs {
                    let (Some(value), Some(options)) = (value.as_str(), input_options(&snapshot.ctx(), node, key)) else {
                        continue;
                    };
                    if !options.is_empty() && !options.iter().any(|o| o == value) {
                        warnings.push(format!("warning: '{}' is not one of the current options for {}: {}", value, key, options.join(", ")));
                    }
                }
            }
        }

        // apply the edit, then add the warnings to the result if it worked
        let mut result = self.apply_edit(|graph, specs, _| edit::set_inputs(graph, specs, &params.node, &params.inputs)).await?;
        if result.is_error != Some(true) {
            for warning in warnings {
                result.content.push(ContentBlock::text(warning));
            }
        }
        Ok(result)
    }

    // graph_remove_node
    #[tool(description = "Remove a node and all its connections.", annotations(read_only_hint = false, destructive_hint = true))]
    async fn graph_remove_node(&self, Parameters(params): Parameters<NodeParams>) -> Result<CallToolResult, McpError> {
        self.apply_edit(|graph, _, _| edit::remove_node(graph, &params.node)).await
    }

    // graph_execute: realtime run, or a full run that writes keyframes to blender
    #[tool(description = "Execute the graph. write_to_blender=false runs the realtime nodes only; true also runs Evaluate Instrument, which writes keyframes into the connected Blender file. Returns the outline with fresh value summaries.", annotations(read_only_hint = false, destructive_hint = true))]
    async fn graph_execute(&self, Parameters(params): Parameters<ExecuteParams>) -> Result<CallToolResult, McpError> {
        let snapshot = match Snapshot::take() {
            Ok(snapshot) => snapshot,
            Err(e) => return tool_error(e),
        };
        // make sure we're allowed to execute: not paused, connected if writing to blender, and not empty
        if snapshot.state.execution_paused {
            return tool_error("execution is paused because Blender scene changes are pending; accept or reject them in the app first");
        }
        if params.write_to_blender && !snapshot.state.connected {
            return tool_error("Blender is not connected; connect it (app_status shows the connection) or run with write_to_blender=false");
        }
        if snapshot.graph.nodes.is_empty() {
            return tool_error("the graph is empty; add nodes with graph_add_node first");
        }

        // run it, realtime is the opposite of writing to blender
        if let Err(e) = run_execution(!params.write_to_blender).await {
            return tool_error(e);
        }

        // take a new snapshot to get the fresh results
        let snapshot = match Snapshot::take() {
            Ok(snapshot) => snapshot,
            Err(e) => return tool_error(e),
        };
        // count how many nodes ran without an error and say what kind of run it was
        let errors = node_errors(&snapshot.state.executed_results);
        let executed = snapshot.state.executed_results.len() - errors.len();
        let mode = if params.write_to_blender {
            "full run, keyframes sent to Blender"
        } else {
            "realtime run"
        };
        let text = match outline::outline(&snapshot.ctx(), None, Detail::Concise) {
            Ok(text) => text,
            Err(e) => return tool_error(e),
        };
        // any failed node makes the whole call an error, with the outline still attached
        if errors.is_empty() {
            ok_text(format!("executed {} of {} nodes ({})\n\n{}", executed, snapshot.graph.nodes.len(), mode, text))
        } else {
            tool_error(format!("{} node(s) failed, the nodes after them didn't run ({})\n{}\n\n{}", errors.len(), mode, errors.join("\n"), text))
        }
    }

    // project_load: replaces the graph and scene data from a .mkproj file
    #[tool(description = "Load a .mkproj project file, replacing the current graph and scene data.", annotations(read_only_hint = false, destructive_hint = true))]
    async fn project_load(&self, Parameters(params): Parameters<PathParams>) -> Result<CallToolResult, McpError> {
        // the frontend has to be loaded before we replace its state
        if !lock_state().ready {
            return tool_error(NOT_READY);
        }
        // load the project into the state
        if let Err(e) = load_project_from(&params.path) {
            return tool_error(format!("could not load '{}': {}", params.path, e));
        }
        // re-run the realtime graph with the loaded project, then outline it
        let execution = run_realtime_if_allowed().await;
        let snapshot = match Snapshot::take() {
            Ok(snapshot) => snapshot,
            Err(e) => return tool_error(e),
        };
        let text = outline::outline(&snapshot.ctx(), None, Detail::Concise).unwrap_or_else(|e| e);
        ok_text(format!("loaded {}: {} nodes, {} edges\n{}\n\n{}", params.path, snapshot.graph.nodes.len(), snapshot.graph.edges.len(), execution, text))
    }

    // project_save: writes the graph and scene data to a .mkproj file
    #[tool(description = "Save the current graph and scene data to a .mkproj file (overwrites it).", annotations(read_only_hint = false, destructive_hint = true))]
    async fn project_save(&self, Parameters(params): Parameters<PathParams>) -> Result<CallToolResult, McpError> {
        // only allow absolute paths ending in .mkproj, so we don't overwrite something by accident
        let path = std::path::Path::new(&params.path);
        if !path.is_absolute() || path.extension().and_then(|e| e.to_str()) != Some("mkproj") {
            return tool_error(format!("'{}' must be an absolute path ending in .mkproj", params.path));
        }
        // write it out
        match save_project_to(&params.path) {
            Ok(path) => ok_text(format!("saved {}", path)),
            Err(e) => tool_error(e),
        }
    }
}

// tells the MCP client the server has tools, its name/version and the instructions
#[tool_handler]
impl ServerHandler for MotionKeysMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(Implementation::new("motionkeys", env!("CARGO_PKG_VERSION"))).with_instructions(INSTRUCTIONS)
    }
}
