// MCP tool definitions. the graph logic lives in `crate::graph::{model, edit, outline}`,
// this file only reads/writes `STATE`, runs execution and formats the results

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::sync::{MutexGuard, PoisonError};

use base64::Engine;
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::graph::edit::{self, EditResult};
use crate::graph::execute::{execute_graph, panic_message};
use crate::graph::builtin::{all_groups, builtin_groups};
use crate::graph::model::{describe_type, dyn_inner, is_param, node_specs, Graph, GroupDef, NodeSpec, Position, Specs};
use crate::graph::run::{instance_path, scoped_values};
use crate::graph::outline::{self, node_block, node_errors, option_warnings, Detail, OutlineCtx};
use crate::graph::history::{EntryInfo, Source, Step};
use crate::graph::ops::{self, graph_in, Ctx, Op};
use crate::graph::tags;
use crate::state::history::{self, Capture};
use crate::state::{open_file, save_project_to, start_instance, update_state, AppState, InstanceState, Opened, STATE};
use crate::ui::screenshot;

// instructions sent to the MCP client when it connects
const INSTRUCTIONS: &str = "MotionKeys: node graphs that turn MIDI into keyframes in a 3D app (Blender, or any app with a MotionKeys bridge). \
Call graph_outline first to see the current graph, and node_types_list for the node types you can add. \
Data flows from outputs to inputs; edit with graph_add_node, graph_connect, graph_set_inputs, graph_disconnect and graph_remove_node. \
Never connect hidden handles: 'par' inputs are set with graph_set_inputs, and outputs marked hidden are display-only. \
Edits show up live in the app and re-run the realtime graph, which never writes to the 3D app; \
the app and these tools share one undo history: graph_history lists it, graph_undo and graph_redo step through it; \
graph_execute with write_keyframes=true writes keyframes to the 3D app. Node ids accept any unique prefix. \
app_screenshot shows what the UI currently looks like. \
Each tab is its own .mkproj file with its own graph, undo history and window layout, like a separate instance of the app; \
every graph tool acts on the tab on screen. app_status lists the tabs, tab_new/tab_switch/tab_close/tab_rename manage them, \
project_load opens a file in a tab of its own, and tab_go_live links the 3D app to one: only the live tab gets the 3D app's scene changes and writes keyframes. \
Frames are labeled boxes around nodes for organizing the graph, they never run: graph_join_in_frame makes one, graph_remove_from_frame takes nodes out, \
graph_set_label names it (or any node) and graph_remove_node removes it, keeping its nodes.";

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
    /// Node group id to work inside (group nodes in graph_outline name theirs); omit for the top-level graph. Editing inside a built-in group gives this project its own copy, shared by every group node using it
    pub group: Option<String>,
}

// node_describe, graph_remove_node
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NodeParams {
    /// Node id or unique id prefix, e.g. "get_midi_file-1"
    pub node: String,
    /// Node group id to work inside (group nodes in graph_outline name theirs); omit for the top-level graph. Editing inside a built-in group gives this project its own copy, shared by every group node using it
    pub group: Option<String>,
}

// types_describe
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TypeParams {
    /// Handle type name as shown in node_types_list, e.g. "Array<MIDINote>"
    pub data_type: String,
}

// app_screenshot
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScreenshotParams {
    /// Window label, e.g. "main", "settings" or a floating panel like "panel-0"; omit for the focused window (main when the app is in the background)
    pub window: Option<String>,
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
    /// Node group id to work inside (group nodes in graph_outline name theirs); omit for the top-level graph. Editing inside a built-in group gives this project its own copy, shared by every group node using it
    pub group: Option<String>,
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
    /// Input handle id on the consuming node; an existing connection into it is replaced, except on a multi input, which keeps every connection
    pub to_input: String,
    /// Node group id to work inside (group nodes in graph_outline name theirs); omit for the top-level graph. Editing inside a built-in group gives this project its own copy, shared by every group node using it
    pub group: Option<String>,
}

// graph_set_tag
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTagParams {
    /// Node (id or unique prefix)
    pub node: String,
    /// "input" or "output"
    pub side: String,
    /// Handle id of the input or output
    pub socket: String,
    /// Tag name; empty removes the tag
    pub name: String,
    /// Node group id to work inside (group nodes in graph_outline name theirs); omit for the top-level graph. Editing inside a built-in group gives this project its own copy, shared by every group node using it
    pub group: Option<String>,
}

// graph_disconnect
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DisconnectParams {
    /// Consuming node (id or unique prefix)
    pub to_node: String,
    /// Input handle id whose connection is removed
    pub to_input: String,
    /// For a multi input: the producing node (id or unique prefix) whose connection is removed; omit to remove every connection into the input
    pub from_node: Option<String>,
    /// For a multi input: the output on from_node whose connection is removed; omit for all of them
    pub from_output: Option<String>,
    /// Node group id to work inside (group nodes in graph_outline name theirs); omit for the top-level graph. Editing inside a built-in group gives this project its own copy, shared by every group node using it
    pub group: Option<String>,
}

// graph_set_inputs
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetInputsParams {
    /// Node id or unique id prefix
    pub node: String,
    /// Input values keyed by input id; merged into the node's values, null unsets one
    pub inputs: Map<String, Value>,
    /// Node group id to work inside (group nodes in graph_outline name theirs); omit for the top-level graph. Editing inside a built-in group gives this project its own copy, shared by every group node using it
    pub group: Option<String>,
}

// graph_join_in_frame
#[derive(Debug, Deserialize, JsonSchema)]
pub struct JoinInFrameParams {
    /// Nodes to put in the new frame (ids or unique prefixes); nodes inside a frame listed here stay in it
    pub nodes: Vec<String>,
    /// The frame's label; omit for none
    pub label: Option<String>,
    /// Node group id to work inside (group nodes in graph_outline name theirs); omit for the top-level graph. Editing inside a built-in group gives this project its own copy, shared by every group node using it
    pub group: Option<String>,
}

// graph_remove_from_frame
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NodesParams {
    /// Node ids or unique prefixes
    pub nodes: Vec<String>,
    /// Node group id to work inside (group nodes in graph_outline name theirs); omit for the top-level graph. Editing inside a built-in group gives this project its own copy, shared by every group node using it
    pub group: Option<String>,
}

// graph_set_label
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetLabelParams {
    /// Node or frame (id or unique prefix)
    pub node: String,
    /// The name shown in its header (a frame's label); empty goes back to none
    pub label: String,
    /// Node group id to work inside (group nodes in graph_outline name theirs); omit for the top-level graph. Editing inside a built-in group gives this project its own copy, shared by every group node using it
    pub group: Option<String>,
}

// graph_execute
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExecuteParams {
    /// false: realtime run (skips nodes that write keyframes). true: full run, writes keyframes to the 3D app
    pub write_keyframes: bool,
}

// project_load, project_save
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PathParams {
    /// Absolute path to a .mkproj project file
    pub path: String,
}

// tab_switch, tab_close, tab_go_live
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TabParams {
    /// Tab id (e.g. "tab-2") or its exact name (e.g. "Graph 2"), as listed by app_status
    pub tab: String,
}

// tab_rename
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenameTabParams {
    /// Tab id (e.g. "tab-2") or its exact name (e.g. "Graph 2"), as listed by app_status
    pub tab: String,
    /// The new label
    pub label: String,
}

// MARK: - State Helpers

/// locks the global state, a poisoned lock (a panic somewhere else) shouldn't take the MCP server down too
fn lock_state() -> MutexGuard<'static, AppState> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// a tab's id from its id or exact name, the error lists the tabs
fn resolve_tab(state: &AppState, tab: &str) -> Result<String, String> {
    let found = state.instance(tab).or_else(|| state.instances.iter().find(|instance| instance.name() == tab));
    found.map(|instance| instance.id.clone()).ok_or_else(|| format!("no tab '{}'; tabs are: {}", tab, state.instances.iter().map(|instance| format!("{} \"{}\"", instance.id, instance.name())).collect::<Vec<_>>().join(", ")))
}

/// a copy of the parts of `AppState` the tools read, with the tab on screen
struct Snapshot {
    state: AppState,
    tab: InstanceState,
    graph: Graph,
    specs: Vec<NodeSpec>,
    groups: BTreeMap<String, GroupDef>,
}

/// one graph of the snapshot: the top level, or inside a node group
struct View<'a> {
    graph: &'a Graph,
    /// the group, `None` at the top level
    scope: Option<&'a GroupDef>,
    /// values recorded for this graph's nodes, keyed by their ids
    results: Cow<'a, HashMap<String, Value>>,
    inputs: Cow<'a, HashMap<String, Value>>,
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
        let tab = state.active().clone();
        let graph = Graph::from_rf(&tab.rf_instance)?;
        let specs = node_specs(&state.default_nodes);
        let groups = all_groups(&graph);
        Ok(Self {
            state,
            tab,
            graph,
            specs,
            groups,
        })
    }

    /// the top-level graph, or the graph inside a node group
    fn view(&self, group: Option<&str>) -> Result<View<'_>, String> {
        let Some(group_id) = group else {
            return Ok(View {
                graph: &self.graph,
                scope: None,
                results: Cow::Borrowed(&self.tab.executed_results),
                inputs: Cow::Borrowed(&self.tab.executed_inputs),
            });
        };
        let def = find_group(&self.groups, group_id)?;
        // values inside a group are recorded under the path of a group node running it
        let path = instance_path(&self.graph, &self.groups, group_id);
        let scoped = |values: &HashMap<String, Value>| path.as_deref().map(|p| scoped_values(values, p)).unwrap_or_default();
        Ok(View {
            graph: &def.graph,
            scope: Some(def),
            results: Cow::Owned(scoped(&self.tab.executed_results)),
            inputs: Cow::Owned(scoped(&self.tab.executed_inputs)),
        })
    }

    /// borrows a view of the snapshot as the context the outline functions need
    fn ctx<'a>(&'a self, view: &'a View<'a>) -> OutlineCtx<'a> {
        OutlineCtx {
            graph: view.graph,
            specs: Specs {
                specs: &self.specs,
                groups: &self.groups,
                scope: view.scope,
            },
            results: &view.results,
            inputs: &view.inputs,
            scene: &self.tab.scene,
        }
    }
}

/// a node group by id, the error lists the ones there are
fn find_group<'a>(groups: &'a BTreeMap<String, GroupDef>, group_id: &str) -> Result<&'a GroupDef, String> {
    groups.get(group_id).ok_or_else(|| format!("no node group '{}'; node groups are: {}", group_id, groups.keys().cloned().collect::<Vec<_>>().join(", ")))
}

/// a note for outlines inside a group whose values aren't recorded
fn group_values_note(view: &View, group: Option<&str>) -> String {
    match group {
        Some(group_id) if view.results.is_empty() => format!("inside node group '{}'. no values recorded inside: a group's inside is only recorded while it's open in the app (or no group node runs it)\n\n", group_id),
        Some(group_id) => format!("inside node group '{}'\n\n", group_id),
        None => String::new(),
    }
}

/// one history entry as a line: `add_node (mcp): added viewer-1 "Viewer"`
fn history_line(entry: &EntryInfo) -> String {
    let source = match entry.source {
        Source::Ui => "app",
        Source::Mcp => "mcp",
    };
    if entry.detail.is_empty() {
        format!("{} ({})", entry.op, source)
    } else {
        format!("{} ({}): {}", entry.op, source, entry.detail)
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

/// runs `execute_graph` in its own task so a panicking executor turns into an error message instead, and a run that
/// didn't happen into its reason
async fn run_execution(realtime: bool) -> Result<(), String> {
    let ran = tokio::spawn(execute_graph(realtime, None)).await.map_err(|e| {
        // pull the panic message out
        if e.is_panic() {
            format!("execution failed: {}", panic_message(&e.into_panic()))
        } else {
            format!("execution failed: {}", e)
        }
    })?;
    ran.map(|_| ())
}

/// runs the realtime graph of the tab on screen unless execution is paused, returns a one-line status
async fn run_realtime_if_allowed() -> String {
    // grab what we need and drop the lock before executing
    let (paused, empty) = {
        let state = lock_state();
        let tab = state.active();
        (tab.execution_paused, Graph::from_rf(&tab.rf_instance).map_or(true, |g| g.nodes.is_empty()))
    };
    // don't run while scene changes are waiting to be reviewed in the app
    if paused {
        return "execution paused (scene changes from the 3D app are pending review in the app); results not updated".to_string();
    }
    if empty {
        return "nothing to execute".to_string();
    }
    // run it, if it fails the last results are still in the state
    if let Err(e) = run_execution(true).await {
        return format!("{}; values below are from the last successful run", e);
    }
    // the edit worked either way, but say which nodes failed
    let errors = node_errors(&lock_state().active().executed_results);
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
    /// applies one edit to the stored graph as an undo step named `op`, pushes it to the UI, re-runs the realtime graph
    /// and reports what changed plus the updated outline of the touched nodes
    async fn apply_edit<F>(&self, op: &str, group: Option<&str>, edit: F) -> Result<CallToolResult, McpError>
    where
        F: FnOnce(&mut Graph, &Specs, &HashMap<String, Value>) -> Result<EditResult, String>,
    {
        // lock the state for the edit only, the lock is dropped before execution
        let result = {
            let mut state = lock_state();
            if !state.ready {
                return tool_error(NOT_READY);
            }
            // parse the graph of the tab on screen out of the state
            let tab = state.active_instance_id.clone();
            let results = &state.active().executed_results;
            let mut graph = match Graph::from_rf(&state.active().rf_instance) {
                Ok(graph) => graph,
                Err(e) => return tool_error(e),
            };
            // apply the edit, only write the graph back to the state if it worked
            let specs = node_specs(&state.default_nodes);
            let groups = all_groups(&graph);
            let outcome = match group {
                None => {
                    let specs = Specs {
                        specs: &specs,
                        groups: &groups,
                        scope: None,
                    };
                    edit(&mut graph, &specs, results).map(|result| {
                        tags::sync(&mut graph);
                        result
                    })
                }
                // edit a copy of the group, it's stored in the project (a built-in becomes the project's own copy)
                Some(group_id) => {
                    let def = match find_group(&groups, group_id) {
                        Ok(def) => def,
                        Err(e) => return tool_error(e),
                    };
                    let results = instance_path(&graph, &groups, group_id).map(|p| scoped_values(results, &p)).unwrap_or_default();
                    let specs = Specs {
                        specs: &specs,
                        groups: &groups,
                        scope: Some(def),
                    };
                    let mut edited = def.clone();
                    let made_local = !graph.groups.contains_key(group_id);
                    edit(&mut edited.graph, &specs, &results).map(|mut result| {
                        tags::sync(&mut edited.graph);
                        graph.groups.insert(group_id.to_string(), edited);
                        if made_local {
                            result.message.push_str(&format!("; node group '{}' is now this project's own copy", group_id));
                        }
                        result
                    })
                }
            };
            match outcome {
                Ok(result) => {
                    // a group whose last group node was removed goes with it, like the editor's edits
                    ops::drop_unused_groups(&mut graph, group);
                    history::commit(&mut state, &tab, graph.to_rf(), Capture::Record(Step::new(op, Source::Mcp).detail(result.message.clone())));
                    result
                }
                Err(e) => return tool_error(e),
            }
        };

        // push the new graph to the UI, then re-run the realtime graph
        update_state();
        let execution = run_realtime_if_allowed().await;
        self.edit_report(group, &result, &execution)
    }

    /// applies ops the way the editor does (`graph::ops`) as one undo step named `op`, pushes the graph to the UI, re-runs
    /// the realtime graph if they changed what it computes and reports the nodes they touched. `build` makes the ops from
    /// the graph they edit (to resolve id prefixes) and says which nodes to report, the ones they add come too
    async fn apply_ops<F>(&self, op: &str, group: Option<&str>, build: F) -> Result<CallToolResult, McpError>
    where
        F: FnOnce(&Graph) -> Result<(Vec<Op>, Vec<String>), String>,
    {
        let (result, affects_output) = {
            let mut state = lock_state();
            if !state.ready {
                return tool_error(NOT_READY);
            }
            let tab = state.active_instance_id.clone();
            let mut project = match Graph::from_rf(&state.active().rf_instance) {
                Ok(graph) => graph,
                Err(e) => return tool_error(e),
            };
            let (mut list, mut touched) = match graph_in(&project, group).and_then(build) {
                Ok(built) => built,
                Err(e) => return tool_error(e),
            };
            // editing a built-in group gives the project its own copy, like the other edits
            let made_local = group.is_some_and(|id| !project.groups.contains_key(id) && builtin_groups().contains_key(id));
            if made_local {
                list.insert(0, Op::MakeLocal);
            }

            // every op on the same copy, only kept if they all worked
            let specs = node_specs(&state.default_nodes);
            let before = project.clone();
            let (results, sizes) = (HashMap::new(), HashMap::new());
            let ctx = Ctx {
                specs: &specs,
                results: &results,
                sizes: &sizes,
            };
            let mut added = Vec::new();
            for o in &list {
                if let Err(e) = ops::apply(&mut project, group, o, &ctx, &mut added) {
                    return tool_error(e);
                }
            }
            let main = list.iter().find(|o| !matches!(o, Op::MakeLocal)).unwrap_or(&list[0]);
            let mut message = format!("{}: {}", op, ops::describe(main, &before, &project, group, &specs, &added));
            if made_local {
                message.push_str(&format!("; node group '{}' is now this project's own copy", group.unwrap()));
            }
            touched.splice(0..0, added.into_iter().map(|a| a.id));
            let affects_output = history::commit(&mut state, &tab, project.to_rf(), Capture::Record(Step::new(op, Source::Mcp).detail(message.clone())));
            (
                EditResult {
                    message,
                    touched,
                },
                affects_output,
            )
        };

        // push the new graph to the UI, frames and labels are only layout and run nothing
        update_state();
        let execution = if affects_output {
            run_realtime_if_allowed().await
        } else {
            "layout only, nothing to run".to_string()
        };
        self.edit_report(group, &result, &execution)
    }

    /// undoes or redoes one step, re-runs the realtime graph and says which step it was
    async fn history_step(&self, redo: bool) -> Result<CallToolResult, McpError> {
        let tab = {
            let state = lock_state();
            if !state.ready {
                return tool_error(NOT_READY);
            }
            state.active_instance_id.clone()
        };
        let Some((entry, _)) = history::step(&tab, redo) else {
            return tool_error(if redo {
                "nothing to redo"
            } else {
                "nothing to undo"
            });
        };
        let execution = run_realtime_if_allowed().await;
        ok_text(format!(
            "{} {}\n{}",
            if redo {
                "redid"
            } else {
                "undid"
            },
            history_line(&entry),
            execution
        ))
    }

    /// builds the tool result for an edit: the message, the execution status and the outline of each touched node
    fn edit_report(&self, group: Option<&str>, result: &EditResult, execution: &str) -> Result<CallToolResult, McpError> {
        // take a fresh snapshot so the outline shows the new results
        let snapshot = match Snapshot::take() {
            Ok(snapshot) => snapshot,
            Err(e) => return tool_error(e),
        };
        let view = match snapshot.view(group) {
            Ok(view) => view,
            Err(e) => return tool_error(e),
        };
        let ctx = snapshot.ctx(&view);
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

    // app_status: ready flag, the bridge connection, graph size and execution state
    #[tool(description = "App status: whether the UI is ready, the 3D app connection (app, version, file), the tabs (which is on screen, which is live), and for the tab on screen its node and edge counts, whether execution is paused and scene changes are pending.", annotations(read_only_hint = true))]
    async fn app_status(&self) -> Result<CallToolResult, McpError> {
        let state = lock_state().clone();
        let tab = state.active();
        // count the nodes and edges, 0 if the graph can't be read
        let (nodes, edges) = Graph::from_rf(&tab.rf_instance).map_or((0, 0), |g| (g.nodes.len(), g.edges.len()));
        // one line per tab
        let tabs: Vec<String> = state
            .instances
            .iter()
            .map(|instance| {
                let mut line = format!("  {} \"{}\"", instance.id, instance.name());
                if let Some(path) = &instance.path {
                    line.push_str(&format!(" {}", path));
                }
                if instance.id == tab.id {
                    line.push_str(" (on screen)");
                }
                if state.connected_instance_id.as_ref() == Some(&instance.id) {
                    line.push_str(if state.connected {
                        " (live)"
                    } else {
                        " (linked, 3D app away)"
                    });
                }
                if instance.unsaved() {
                    line.push_str(" (unsaved)");
                }
                line
            })
            .collect();
        // describe the bridge connection
        let connection = if state.connected {
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
        ok_text(format!("ready: {}\n3d app: {}\ntabs:\n{}\ngraph on screen: {} nodes, {} edges\nexecution paused: {}\npending scene changes: {}\nkeyframes waiting to be imported (over the limit, the user confirms in the app): {}", state.ready, connection, tabs.join("\n"), nodes, edges, tab.execution_paused, tab.pending_scene.is_some(), tab.pending_curve_import.as_ref().map_or("none".to_string(), |objects| objects.iter().map(|(name, count)| format!("{name} ({count} keyframes)")).collect::<Vec<_>>().join(", "))))
    }

    // node_types_list: every node type with its inputs and outputs
    #[tool(description = "List the node types that can be added, with their inputs and outputs (id, UI name, type, description). 'par' inputs are hidden in the UI: set them with graph_set_inputs, never connect them. Outputs marked hidden are display-only and must not be connected either.", annotations(read_only_hint = true))]
    async fn node_types_list(&self) -> Result<CallToolResult, McpError> {
        let state = lock_state().clone();
        let specs = node_specs(&state.default_nodes);
        let graph = Graph::from_rf(&state.active().rf_instance).unwrap_or_default();
        let groups = all_groups(&graph);
        let lookup = Specs {
            specs: &specs,
            groups: &groups,
            scope: None,
        };
        // node groups are added by their id, the plain group type is only how they're stored
        let group_specs: Vec<NodeSpec> = groups
            .keys()
            .filter_map(|id| {
                lookup.for_type(id).map(|spec| NodeSpec {
                    id: id.clone(),
                    ..spec.into_owned()
                })
            })
            .collect();
        // frames are made with graph_join_in_frame
        let all: Vec<&NodeSpec> = specs.iter().filter(|spec| spec.id != "group" && spec.id != "frame").chain(group_specs.iter()).collect();
        // one block per node type
        let blocks: Vec<String> = all
            .into_iter()
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
                        "  [not realtime: runs only with graph_execute write_keyframes=true]"
                    }
                )];
                // one line per input, parameters (hidden inputs) are marked `par`
                for input in &spec.handles.inputs {
                    // a `Dyn<T>` input grows numbered inputs as they get connected
                    if let Some(inner) = dyn_inner(input) {
                        lines.push(format!("  in   {0}_0, {0}_1, ... \"{1}\": {2} — {3} Dynamic: connect to {0}_0, each connection adds the next free input.", input.id, input.name, inner, input.description));
                        continue;
                    }
                    let kind = if is_param(spec, &input.id) {
                        "par"
                    } else {
                        "in "
                    };
                    // a multi input keeps every connection
                    let multi = if input.multi {
                        " [multi: takes any number of connections]"
                    } else {
                        ""
                    };
                    lines.push(format!("  {}  {} \"{}\": {}{} — {}", kind, input.id, input.name, input.data_type, multi, input.description));
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
        let view = match snapshot.view(params.group.as_deref()) {
            Ok(view) => view,
            Err(e) => return tool_error(e),
        };
        // add the node and edge counts on top of the outline
        match outline::outline(&snapshot.ctx(&view), params.scope.as_deref(), detail) {
            Ok(text) => ok_text(format!("{}{} nodes, {} edges\n\n{}", group_values_note(&view, params.group.as_deref()), view.graph.nodes.len(), view.graph.edges.len(), text)),
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
        let view = match snapshot.view(params.group.as_deref()) {
            Ok(view) => view,
            Err(e) => return tool_error(e),
        };
        // resolve the id (prefixes are allowed) and show it in full detail
        match view.graph.resolve(&params.node) {
            Ok(id) => ok_text(node_block(&snapshot.ctx(&view), &id, Detail::Full)),
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

    // app_screenshot: PNG of a window's contents
    #[tool(description = "Screenshot of a MotionKeys window as a PNG, for checking what the UI shows. Captures the window as drawn on screen, including the title bar (on Linux, the page only without the title bar), and works while it is covered or the app is in the background. Also lists the window labels that can be captured.", annotations(read_only_hint = true))]
    async fn app_screenshot(&self, Parameters(params): Parameters<ScreenshotParams>) -> Result<CallToolResult, McpError> {
        // list the windows so the next call can pick a specific one
        let windows: Vec<String> = screenshot::window_labels()
            .into_iter()
            .map(|(label, focused)| {
                if focused {
                    format!("{} (focused)", label)
                } else {
                    label
                }
            })
            .collect();
        let shot = match screenshot::capture_window(params.window.as_deref()).await {
            Ok(shot) => shot,
            Err(e) => return tool_error(format!("{}\nwindows: {}", e, windows.join(", "))),
        };
        // image first, then which window it is
        let data = base64::engine::general_purpose::STANDARD.encode(&shot.png);
        Ok(CallToolResult::success(vec![ContentBlock::image(data, "image/png"), ContentBlock::text(format!("window '{}', {}x{} px\nwindows: {}", shot.label, shot.width, shot.height, windows.join(", ")))]))
    }

    // MARK: - Write Tools

    // graph_add_node
    #[tool(description = "Add a node. Returns its new id (e.g. get_midi_file-2). Without position it is placed right of 'after', or right of the right-most node.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_add_node(&self, Parameters(params): Parameters<AddNodeParams>) -> Result<CallToolResult, McpError> {
        // add the node through apply_edit so the UI and realtime results get updated
        self.apply_edit("add_node", params.group.as_deref(), |graph, specs, _| edit::add_node(graph, specs, &params.node_type, params.inputs.as_ref(), params.position, params.after.as_deref())).await
    }

    // graph_connect
    #[tool(description = "Connect an output to an input (data flows from_node.from_output -> to_node.to_input). Checks types and cycles; replaces any existing connection into that input, except on a multi input, which takes any number of connections and gets them as one list. Hidden handles ('par' inputs and outputs marked hidden) must never be connected and are refused.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_connect(&self, Parameters(params): Parameters<ConnectParams>) -> Result<CallToolResult, McpError> {
        self.apply_edit("connect", params.group.as_deref(), |graph, specs, results| edit::connect(graph, specs, results, &params.from_node, &params.from_output, &params.to_node, &params.to_input)).await
    }

    // graph_disconnect
    #[tool(description = "Remove the connection into one input; for a multi input, the ones from from_node (and from_output), or all of them.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_disconnect(&self, Parameters(params): Parameters<DisconnectParams>) -> Result<CallToolResult, McpError> {
        self.apply_edit("disconnect", params.group.as_deref(), |graph, _, _| edit::disconnect(graph, &params.to_node, &params.to_input, params.from_node.as_deref(), params.from_output.as_deref())).await
    }

    // graph_set_tag
    #[tool(
        description = "Signal tags connect sockets by name instead of a wire (the UI draws a tag next to each socket, no wire). An output's tag is the source of that name in its graph, one output per name; every input with the same tag takes its value, as an ordinary connection. An input whose tag no output has is broken and unconnected until one does. Tagging an untagged output turns its wires into tags; renaming an output's tag renames it on every input using it; an empty name removes the tag and turns its connections back into wires. Tagging a wired input with a new name tags the wire's output too. graph_connect and graph_disconnect on an input remove its tag. Tags only connect inside one graph (the top level or one group).",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    async fn graph_set_tag(&self, Parameters(params): Parameters<SetTagParams>) -> Result<CallToolResult, McpError> {
        self.apply_edit("set_tag", params.group.as_deref(), |graph, specs, results| edit::set_tag(graph, specs, results, &params.node, &params.side, &params.socket, &params.name)).await
    }

    // graph_set_inputs
    #[tool(description = "Set input values on a node (parameters like file_path or track_name, or unconnected inputs). Keys must be declared input ids; values are merged, null unsets.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_set_inputs(&self, Parameters(params): Parameters<SetInputsParams>) -> Result<CallToolResult, McpError> {
        // warn about values that aren't one of the currently known options
        let mut warnings: Vec<String> = Vec::new();
        let snapshot = Snapshot::take();
        let view = snapshot.as_ref().ok().and_then(|s| s.view(params.group.as_deref()).ok());
        if let (Ok(snapshot), Some(view)) = (&snapshot, &view) {
            if let Some(node) = view.graph.resolve(&params.node).ok().and_then(|id| view.graph.node(&id)) {
                warnings = option_warnings(&snapshot.ctx(view), node, &params.inputs);
            }
        }

        // apply the edit, then add the warnings to the result if it worked
        let mut result = self.apply_edit("set_inputs", params.group.as_deref(), |graph, specs, _| edit::set_inputs(graph, specs, &params.node, &params.inputs)).await?;
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
        self.apply_edit("remove_node", params.group.as_deref(), |graph, _, _| edit::remove_node(graph, &params.node)).await
    }

    // graph_join_in_frame
    #[tool(description = "Put nodes in a new frame: a labeled box around them for organizing the graph, it never runs. The frame fits around its nodes and goes inside the innermost frame they share. Returns the frame's id.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_join_in_frame(&self, Parameters(params): Parameters<JoinInFrameParams>) -> Result<CallToolResult, McpError> {
        self.apply_ops("join_in_frame", params.group.as_deref(), |graph| {
            let nodes = params.nodes.iter().map(|n| graph.resolve(n)).collect::<Result<Vec<_>, _>>()?;
            let mut list = vec![Op::JoinInFrame {
                nodes: nodes.clone(),
            }];
            // the new frame's id is the next one, labeled in the same step
            if let Some(label) = params.label.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
                list.push(Op::SetLabel {
                    node: graph.next_node_id("frame"),
                    label: label.to_string(),
                });
            }
            Ok((list, nodes))
        })
        .await
    }

    // graph_remove_from_frame
    #[tool(description = "Take nodes out of their frames, into the frame around it if there is one. A frame left with nothing in it is removed.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_remove_from_frame(&self, Parameters(params): Parameters<NodesParams>) -> Result<CallToolResult, McpError> {
        self.apply_ops("remove_from_frame", params.group.as_deref(), |graph| {
            let nodes = params.nodes.iter().map(|n| graph.resolve(n)).collect::<Result<Vec<_>, _>>()?;
            Ok((
                vec![Op::RemoveFromFrame {
                    nodes: nodes.clone(),
                }],
                nodes,
            ))
        })
        .await
    }

    // graph_set_label
    #[tool(description = "Set the label of a frame or a node (shown in its header instead of its type's name); an empty label removes it.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_set_label(&self, Parameters(params): Parameters<SetLabelParams>) -> Result<CallToolResult, McpError> {
        self.apply_ops("set_label", params.group.as_deref(), |graph| {
            let node = graph.resolve(&params.node)?;
            Ok((
                vec![Op::SetLabel {
                    node: node.clone(),
                    label: params.label.trim().to_string(),
                }],
                vec![node],
            ))
        })
        .await
    }

    // MARK: - History Tools

    // graph_history: the undo history shared with the app
    #[tool(description = "List the undo history of the tab on screen, oldest first. The app and MCP share it: entries from the app are the user's own edits. Entries after the current position can be redone.", annotations(read_only_hint = true))]
    async fn graph_history(&self) -> Result<CallToolResult, McpError> {
        let tab = lock_state().active_instance_id.clone();
        let info = history::info(&tab);
        if info.entries.is_empty() {
            return ok_text("the history is empty");
        }
        let mut text = format!("{} of {} steps done (oldest first):\n", info.current, info.entries.len());
        for (i, entry) in info.entries.iter().enumerate() {
            if i == info.current {
                text.push_str("-- undone, can be redone --\n");
            }
            text.push_str(&history_line(entry));
            text.push('\n');
        }
        ok_text(text)
    }

    // graph_undo
    #[tool(description = "Undo the newest step in the undo history. The history is shared with the app, so this can undo the user's own edits; check graph_history first.", annotations(read_only_hint = false, destructive_hint = true))]
    async fn graph_undo(&self) -> Result<CallToolResult, McpError> {
        self.history_step(false).await
    }

    // graph_redo
    #[tool(description = "Redo the newest undone step in the undo history.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn graph_redo(&self) -> Result<CallToolResult, McpError> {
        self.history_step(true).await
    }

    // graph_execute: realtime run, or a full run that writes keyframes to the 3D app
    #[tool(description = "Execute the graph. write_keyframes=false runs the realtime nodes only; true also runs Evaluate Instrument, which writes keyframes into the connected 3D app's file. Returns the outline with fresh value summaries.", annotations(read_only_hint = false, destructive_hint = true))]
    async fn graph_execute(&self, Parameters(params): Parameters<ExecuteParams>) -> Result<CallToolResult, McpError> {
        let snapshot = match Snapshot::take() {
            Ok(snapshot) => snapshot,
            Err(e) => return tool_error(e),
        };
        // make sure we're allowed to execute: not paused, connected if writing keyframes, and not empty
        if snapshot.tab.execution_paused {
            return tool_error("execution is paused because scene changes from the 3D app are pending; accept or reject them in the app first");
        }
        if params.write_keyframes && !snapshot.state.connected {
            return tool_error("no 3D app is connected; connect its bridge (app_status shows the connection) or run with write_keyframes=false");
        }
        if params.write_keyframes && !snapshot.state.is_live(&snapshot.tab.id) {
            return tool_error("the tab on screen isn't live, only the live tab writes keyframes; switch to it or link this one with tab_go_live (app_status shows which is live)");
        }
        if snapshot.graph.nodes.is_empty() {
            return tool_error("the graph is empty; add nodes with graph_add_node first");
        }

        // run it, realtime is the opposite of writing keyframes
        if let Err(e) = run_execution(!params.write_keyframes).await {
            return tool_error(e);
        }

        // take a new snapshot to get the fresh results
        let snapshot = match Snapshot::take() {
            Ok(snapshot) => snapshot,
            Err(e) => return tool_error(e),
        };
        // count how many nodes ran without an error and say what kind of run it was
        let errors = node_errors(&snapshot.tab.executed_results);
        let executed = snapshot.tab.executed_results.len() - errors.len();
        let mode = if params.write_keyframes {
            "full run, keyframes sent to the 3D app"
        } else {
            "realtime run"
        };
        let view = snapshot.view(None).unwrap();
        let text = match outline::outline(&snapshot.ctx(&view), None, Detail::Concise) {
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

    // project_load: opens a .mkproj file in a tab of its own
    #[tool(description = "Open a .mkproj project file in a tab of its own and show it: it takes the place of the tab on screen if that one is an untouched empty graph, otherwise it's added after the others. A file that's already open in a tab is read again from disk into a new tab in that tab's place (its undo history starts over); if that tab has unsaved changes the call fails instead, so they aren't lost. The 3D app links to it if no other tab is live.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn project_load(&self, Parameters(params): Parameters<PathParams>) -> Result<CallToolResult, McpError> {
        // the frontend has to be loaded before we change its state
        if !lock_state().ready {
            return tool_error(NOT_READY);
        }
        // the app asks before unsaved changes are read over, this can't
        if let Some(tab) = lock_state().unsaved_file_tab(&params.path) {
            return tool_error(format!("'{}' is open in {} with unsaved changes, opening it again would drop them: save them first (tab_switch, then project_save), or close the tab with tab_close to drop them", params.path, tab));
        }
        // open it in a tab, then link the 3D app to it or run it
        let opened = match open_file(&params.path) {
            Ok(opened) => opened,
            Err(e) => return tool_error(format!("could not open '{}': {}", params.path, e)),
        };
        let id = opened.id().to_string();
        start_instance(id.clone()).await;
        // outline it with the fresh results
        let execution = run_realtime_if_allowed().await;
        let snapshot = match Snapshot::take() {
            Ok(snapshot) => snapshot,
            Err(e) => return tool_error(e),
        };
        let view = snapshot.view(None).unwrap();
        let text = outline::outline(&snapshot.ctx(&view), None, Detail::Concise).unwrap_or_else(|e| e);
        let how = match &opened {
            Opened::New(_) => "opened in".to_string(),
            Opened::Reopened {
                replaced,
                ..
            } => format!("was open in {}, read again into", replaced),
        };
        ok_text(format!("{} {} {} \"{}\" (on screen): {} nodes, {} edges\n{}\n\n{}", params.path, how, id, snapshot.tab.name(), snapshot.graph.nodes.len(), snapshot.graph.edges.len(), execution, text))
    }

    // project_save: writes the tab on screen to a .mkproj file
    #[tool(description = "Save the tab on screen (its graph, scene data and layout) to a .mkproj file, which becomes the tab's file and name (overwrites it).", annotations(read_only_hint = false, destructive_hint = true))]
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

    // MARK: - Tab Tools

    // tab_new
    #[tool(description = "Add an empty tab after the others and show it. Graph tools then act on it.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn tab_new(&self) -> Result<CallToolResult, McpError> {
        if !lock_state().ready {
            return tool_error(NOT_READY);
        }
        let id = crate::state::create_instance();
        let label = lock_state().instance(&id).map(InstanceState::name).unwrap_or_default();
        ok_text(format!("added {} \"{}\", it's on screen", id, label))
    }

    // tab_switch
    #[tool(description = "Show a tab. Graph tools then act on it; it re-runs the realtime graph.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn tab_switch(&self, Parameters(params): Parameters<TabParams>) -> Result<CallToolResult, McpError> {
        let id = match resolve_tab(&lock_state(), &params.tab) {
            Ok(id) => id,
            Err(e) => return tool_error(e),
        };
        crate::state::switch_active_instance(id.clone()).await;
        ok_text(format!("{} is on screen", id))
    }

    // tab_close
    #[tool(description = "Close a tab without asking, its graph and undo history are gone (unless the project file has them). The last tab can't be closed. The 3D app unlinks if it was the live tab.", annotations(read_only_hint = false, destructive_hint = true))]
    async fn tab_close(&self, Parameters(params): Parameters<TabParams>) -> Result<CallToolResult, McpError> {
        let id = match resolve_tab(&lock_state(), &params.tab) {
            Ok(id) => id,
            Err(e) => return tool_error(e),
        };
        if !crate::state::close_instance(id.clone()).await {
            return tool_error("the last tab can't be closed");
        }
        let shown = lock_state().active_instance_id.clone();
        ok_text(format!("closed {}, {} is on screen", id, shown))
    }

    // tab_rename
    #[tool(description = "Rename a tab that hasn't been saved yet (a saved tab is named after its file); the name is what the save dialog suggests.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn tab_rename(&self, Parameters(params): Parameters<RenameTabParams>) -> Result<CallToolResult, McpError> {
        let id = match resolve_tab(&lock_state(), &params.tab) {
            Ok(id) => id,
            Err(e) => return tool_error(e),
        };
        if params.label.trim().is_empty() {
            return tool_error("the label can't be blank");
        }
        if !crate::state::rename_instance(id.clone(), params.label.clone()) {
            return tool_error("a saved tab is named after its file and can't be renamed");
        }
        ok_text(format!("renamed {} to \"{}\"", id, params.label.trim()))
    }

    // tab_go_live
    #[tool(description = "Link the connected 3D app to a tab (it goes live): it gets the 3D app's scene changes and is the only tab that writes keyframes. When the 3D app's objects or groups differ from the tab's saved scene, the tab pauses until the user reviews the changes in the app.", annotations(read_only_hint = false, destructive_hint = false))]
    async fn tab_go_live(&self, Parameters(params): Parameters<TabParams>) -> Result<CallToolResult, McpError> {
        let id = match resolve_tab(&lock_state(), &params.tab) {
            Ok(id) => id,
            Err(e) => return tool_error(e),
        };
        if let Err(e) = crate::state::link(id.clone()).await {
            return tool_error(e);
        }
        let paused = lock_state().instance(&id).is_some_and(|instance| instance.execution_paused);
        ok_text(if paused {
            format!("{} is live; the 3D app's scene differs from the tab's, so it's paused until the changes are reviewed in the app", id)
        } else {
            format!("{} is live", id)
        })
    }
}

// tells the MCP client the server has tools, its name/version and the instructions
#[tool_handler]
impl ServerHandler for MotionKeysMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(Implementation::new("motionkeys", env!("CARGO_PKG_VERSION"))).with_instructions(INSTRUCTIONS)
    }
}
