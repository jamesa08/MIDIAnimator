// runs node graphs. a graph runs its nodes in dependency order, a group node runs its group's graph the same way
// (recursively), and a for each zone runs the nodes inside it once per item.
//
// ZONES: a `for_each_input` and `for_each_output` pair (linked by `data.zone`, the id of the other node) wraps the
// nodes between them, like Blender's zones. a node is inside the zone if it depends on the zone input and the zone
// output doesn't come before it. nodes inside can read anything outside, but only the zone output carries values out.
//
// SPEED: nodes pass typed values (`Val`), JSON is only made for the record the UI shows. each graph is turned into a
// `Plan` once per run (order, connections, values set on nodes, zones), and the `Memo` keeps what every node gave
// last run: a node whose settings and input values (the same values, not just equal ones) haven't changed isn't
// run again, and neither is a zone. the graph shows which nodes bring in the outside world: a node with nothing
// connected (a file, the Blender scene) always runs, and keeps its old values when they come out the same so the
// nodes after it still hit the memo. a node that isn't realtime (writing to Blender) always runs
//
// STALE WRITES: the memo also keeps the inputs of each node writing to Blender's last write that went through. a run
// that skips the node (realtime) compares what it would get now with them: the same values, or equal ones (compared
// by a fingerprint of their JSON, values made again upstream are new values even when they come out equal), mean
// Blender still has what the graph gives. the record lists those nodes as fresh, every other one is stale
//
// TYPES: anything can be connected to anything in the editor, the run checks it. a connection whose output type
// doesn't fit its input (`model::compatible`) fails the node it goes into without running it, and so does a value
// that can't be converted to what the node asked for. a failed node records which inputs were bad.
// a node that's only missing a required input isn't an error, it waits: it isn't shown as failed and the nodes
// after it don't run
//
// TIMINGS: every node that came up in a run gets a `NodeTiming` in the record: whether it ran or came from the memo, how
// long it took and how it went. a node inside a zone has one for all of its items together, a group node's time
// includes its inside. measuring is a clock read before and after each node, nothing else changes

use serde_json::{json, Map, Value};
use std::cell::{OnceCell, RefCell};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::Hasher;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::executors::io::{Conversions, Inputs, NodeFunction, Outputs, Val, BAD_INPUTS_KEY, ERROR_KEY};
use super::model::{compatible, dyn_index, dyn_inner, find_spec, Graph, GroupDef, HandleSpec, NodeSpec, RfNode, Specs};

pub const GROUP: &str = "group";
pub const GROUP_INPUT: &str = "group_input";
pub const GROUP_OUTPUT: &str = "group_output";
pub const FOR_EACH_INPUT: &str = "for_each_input";
pub const FOR_EACH_OUTPUT: &str = "for_each_output";
/// a frame around nodes (`graph::frames`), only layout: it never runs
pub const FRAME: &str = "frame";

/// separates the ids in a node path, `group-1/viewer-2` is `viewer-2` inside the group node `group-1`
pub const PATH_SEP: char = '/';

/// input or output id to value
type Values = Vec<(Arc<str>, Val)>;

/// input id to what's wrong with the value it got
type BadInputs = Vec<(Arc<str>, String)>;

// MARK: - Context

/// what one run shares: the node types, groups and settings, and the memo from the last run
pub struct RunCtx<'a> {
    pub specs: &'a [NodeSpec],
    pub registry: &'a HashMap<String, NodeFunction>,
    pub groups: &'a BTreeMap<String, GroupDef>,
    /// path of the group open in the editor. only it and the groups around it record their insides, nobody sees the rest
    pub inspect: String,
    /// realtime runs skip nodes that aren't marked realtime (scene_writer)
    pub realtime: bool,
    memo: RefCell<Memo>,
    conversions: Conversions,
    plans: RefCell<HashMap<String, Rc<Plan>>>,
    /// what the nodes writing to Blender did, kept even inside a group that isn't open so the group node can show it
    writes: RefCell<Vec<(String, Write)>>,
    /// how each node went, and where each path is in it (a group inside a zone runs once per item)
    timings: RefCell<(Vec<NodeTiming>, HashMap<String, usize>)>,
}

/// what a node writing to Blender (not realtime) did in a run
enum Write {
    /// a realtime run skipped it, `fresh` if Blender still has what it would write now
    Skipped {
        fresh: bool,
    },
    /// it ran, with its error if it failed
    Ran(Option<String>),
}

impl<'a> RunCtx<'a> {
    pub fn new(specs: &'a [NodeSpec], registry: &'a HashMap<String, NodeFunction>, groups: &'a BTreeMap<String, GroupDef>, realtime: bool) -> Self {
        Self {
            specs,
            registry,
            groups,
            inspect: String::new(),
            realtime,
            memo: RefCell::new(Memo::default()),
            conversions: Conversions::default(),
            plans: RefCell::new(HashMap::new()),
            writes: RefCell::new(Vec::new()),
            timings: RefCell::new((Vec::new(), HashMap::new())),
        }
    }

    /// start from what a previous run left
    pub fn with_memo(self, mut memo: Memo) -> Self {
        memo.hits = 0;
        *self.memo.borrow_mut() = memo;
        self
    }

    /// what this run leaves for the next one, without anything this run didn't use
    pub fn into_memo(self) -> Memo {
        let mut memo = self.memo.into_inner();
        let (seen_nodes, seen_json, seen_literals) = (std::mem::take(&mut memo.seen_nodes), std::mem::take(&mut memo.seen_json), std::mem::take(&mut memo.seen_literals));
        memo.nodes.retain(|path, _| seen_nodes.contains(path));
        memo.json.retain(|address, _| seen_json.contains(address));
        memo.literals.retain(|json, _| seen_literals.contains(json));
        memo
    }

    /// adds a node's timing, a path that already has one (a group inside a zone, once per item) adds up with it
    fn add_timing(&self, timing: NodeTiming) {
        let (timings, index) = &mut *self.timings.borrow_mut();
        let Some(&at) = index.get(&timing.node) else {
            index.insert(timing.node.clone(), timings.len());
            timings.push(timing);
            return;
        };
        let into = &mut timings[at];
        into.ms += timing.ms;
        into.cached &= timing.cached;
        into.iterations = match (into.iterations, timing.iterations) {
            (Some(a), Some(b)) => Some(a + b),
            (a, b) => a.or(b),
        };
        if into.status != Status::Failed {
            into.status = timing.status;
            into.error = timing.error;
            into.own_error = timing.own_error;
        }
    }

    /// the plan for a graph, each group is planned once per run. frames are only layout, they're left out
    fn plan(&self, graph: &Graph, group_id: Option<&str>) -> Rc<Plan> {
        let frameless;
        let graph = if graph.nodes.iter().any(RfNode::is_frame) {
            frameless = Graph {
                nodes: graph.nodes.iter().filter(|n| !n.is_frame()).cloned().collect(),
                edges: graph.edges.clone(),
                ..Default::default()
            };
            &frameless
        } else {
            graph
        };
        let Some(group_id) = group_id else {
            return Rc::new(Plan::new(self, graph, None));
        };
        if let Some(plan) = self.plans.borrow().get(group_id) {
            return plan.clone();
        }
        let plan = Rc::new(Plan::new(self, graph, self.groups.get(group_id)));
        self.plans.borrow_mut().insert(group_id.to_string(), plan.clone());
        plan
    }
}

// MARK: - Memo

/// what nodes gave in earlier runs, kept between runs by `execute_graph`
#[derive(Default)]
pub struct Memo {
    nodes: HashMap<String, NodeMemo>,
    /// JSON of values already shown in the record, by value
    json: HashMap<usize, (Val, Value)>,
    /// values set on nodes by their JSON, the same JSON gives the same value every run so nodes given it still match
    literals: HashMap<String, Val>,
    seen_nodes: HashSet<String>,
    seen_json: HashSet<usize>,
    seen_literals: HashSet<String>,
    hits: usize,
    /// what each node writing to Blender was given in its last write that went through, by path
    written: HashMap<String, Written>,
}

/// the inputs of a write that went through
struct Written {
    key: Values,
    /// made the first time other values are compared with these
    fingerprint: OnceCell<u64>,
}

struct NodeMemo {
    /// the node's type and the values set on it, for a zone every node in it
    signature: String,
    /// the values it was given, for a zone also what the inside reads from outside
    key: Values,
    outputs: Result<Outputs, String>,
    bad_inputs: BadInputs,
    waiting: bool,
    /// for a zone: what its nodes recorded for the first item, and its output node
    record: Vec<Entry>,
}

impl Memo {
    /// how many nodes and zones the last run took from the memo instead of running them
    pub fn hits(&self) -> usize {
        self.hits
    }

    /// what the node at `path` gave last time, if it was given the same values
    fn hit(&mut self, path: &str, signature: &str, key: &Values) -> Option<&NodeMemo> {
        self.seen_nodes.insert(path.to_string());
        let memo = self.nodes.get(path)?;
        let same = memo.signature == signature && memo.key.len() == key.len() && memo.key.iter().zip(key).all(|((k1, v1), (k2, v2))| k1 == k2 && v1.same(v2));
        if same {
            self.hits += 1;
        }
        same.then_some(memo)
    }

    /// true if the node writing to Blender at `path` would write what its last write did with `key`. equal values that
    /// aren't the same ones take their place, so the next run can compare them by pointer again
    fn still_written(&mut self, path: &str, key: &Values) -> bool {
        let Some(written) = self.written.get_mut(path) else {
            return false;
        };
        let same = written.key.len() == key.len() && written.key.iter().zip(key).all(|((k1, v1), (k2, v2))| k1 == k2 && v1.same(v2));
        if same {
            return true;
        }
        let equal = *written.fingerprint.get_or_init(|| fingerprint(&written.key)) == fingerprint(key);
        if equal {
            written.key = key.clone();
        }
        equal
    }

    /// a node writing to Blender wrote `key`, or failed (`None`) and Blender may have anything
    fn set_written(&mut self, path: &str, key: Option<Values>) {
        match key {
            Some(key) => self.written.insert(
                path.to_string(),
                Written {
                    key,
                    fingerprint: OnceCell::new(),
                },
            ),
            None => self.written.remove(path),
        };
    }

    fn store(&mut self, path: &str, memo: NodeMemo) {
        self.seen_nodes.insert(path.to_string());
        self.nodes.insert(path.to_string(), memo);
    }

    /// the value for a value set on a node, the same one as last run if the JSON is the same
    fn literal(&mut self, value: &Value) -> Val {
        let json = value.to_string();
        self.seen_literals.insert(json.clone());
        self.literals.entry(json).or_insert_with(|| Val::json(value.clone())).clone()
    }

    /// a value as JSON, made once per value
    fn json(&mut self, value: &Val) -> Value {
        let address = value.address();
        self.seen_json.insert(address);
        if let Some((_, json)) = self.json.get(&address) {
            return json.clone();
        }
        let json = value.to_json();
        self.json.insert(address, (value.clone(), json.clone()));
        json
    }
}

/// a hash of values' JSON, equal values give the same one. the JSON is streamed into the hash, never kept
fn fingerprint(values: &Values) -> u64 {
    struct HashWriter(std::collections::hash_map::DefaultHasher);
    impl std::io::Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.write(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = HashWriter(Default::default());
    for (key, value) in values {
        writer.0.write(key.as_bytes());
        let _ = serde_json::to_writer(&mut writer, &value.to_json());
    }
    writer.0.finish()
}

// MARK: - Record

/// one node's values for the record, turned into JSON at the end. `outputs` is `None` for a node that didn't run
/// (not realtime), only its inputs are shown
#[derive(Clone)]
struct Entry {
    path: String,
    outputs: Option<Result<Outputs, String>>,
    inputs: Values,
    /// for a failed node, the inputs that got the wrong type
    bad_inputs: BadInputs,
}

/// the results and inputs of every node that ran, keyed by node path, what the UI and the MCP outline show.
/// inside a for each zone only the first item is kept
#[derive(Default, Debug)]
pub struct Record {
    pub results: HashMap<String, Value>,
    pub inputs: HashMap<String, Value>,
    /// nodes writing to Blender a realtime run skipped (also inside a group that isn't open), they have no results of their own
    pub skipped: Vec<String>,
    /// nodes writing to Blender that Blender has the latest of: written in this run, or skipped with what their last
    /// write had. the other ones in the graph are stale
    pub fresh: Vec<String>,
    /// how each node that came up went, in the order they came up (inside groups and zones too, open or not)
    pub timings: Vec<NodeTiming>,
}

/// how a node went in a run
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Failed,
    /// missing a required input
    Waiting,
    /// a realtime run skipped it, it writes to Blender
    Skipped,
}

/// how long a node took in a run and how it went. `node`: its path. `cached`: it came from the memo every time it came
/// up. `ms`: wall time, for a group node with its inside, inside a zone for every item. `group`: path of the group node
/// it's inside. `zone`: path of the for each output of the zone it's inside. `iterations`: a zone's items, or how many
/// times a node inside a zone came up
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct NodeTiming {
    pub node: String,
    pub node_type: String,
    pub label: String,
    pub cached: bool,
    pub ms: f64,
    pub status: Status,
    pub error: Option<String>,
    pub group: Option<String>,
    pub zone: Option<String>,
    pub iterations: Option<u32>,
    /// the error started at this node, not inside the group it runs or one of its zone's items
    #[serde(skip)]
    pub own_error: bool,
}

/// runs the root graph, returns the record and the first error (the rest of the graph still ran)
pub fn run(ctx: &RunCtx, graph: &Graph) -> (Record, Option<String>) {
    let mut entries = Vec::new();
    let error = run_graph(ctx, graph, None, "", &Outputs::new(), &mut Vec::new(), &mut Some(&mut entries)).err();

    // JSON for the UI, each value is only turned into JSON once (also across runs)
    let mut memo = ctx.memo.borrow_mut();
    let mut record = Record::default();
    for entry in entries {
        let inputs = Value::Object(entry.inputs.iter().map(|(k, v)| (k.to_string(), memo.json(v))).collect::<Map<_, _>>());
        record.inputs.insert(entry.path.clone(), inputs);
        match &entry.outputs {
            Some(Ok(outputs)) => record.results.insert(entry.path, Value::Object(outputs.iter().map(|(k, v)| (k.to_string(), memo.json(v))).collect())),
            Some(Err(message)) => {
                let mut failed = json!({ ERROR_KEY: message });
                if !entry.bad_inputs.is_empty() {
                    failed[BAD_INPUTS_KEY] = Value::Object(entry.bad_inputs.iter().map(|(k, m)| (k.to_string(), Value::String(m.clone()))).collect());
                }
                record.results.insert(entry.path, failed)
            }
            None => None,
        };
    }
    // writes inside a group that isn't open weren't recorded above
    for (path, write) in ctx.writes.take() {
        match write {
            Write::Skipped {
                fresh,
            } => {
                if fresh {
                    record.fresh.push(path.clone());
                }
                record.skipped.push(path);
            }
            Write::Ran(error) => {
                if error.is_none() {
                    record.fresh.push(path.clone());
                }
                record.results.entry(path).or_insert_with(|| error.map_or_else(|| json!({}), |message| json!({ ERROR_KEY: message })));
            }
        }
    }
    record.timings = std::mem::take(&mut ctx.timings.borrow_mut().0);
    (record, error)
}

// MARK: - Plan

/// what a node does when it runs
enum Kind {
    Executor(NodeFunction),
    GroupInput,
    GroupOutput,
    Group(String),
    ZoneEnd,
    Broken(String),
}

struct PlanNode {
    id: String,
    /// its type and what it's called, for its timing
    node_type: String,
    label: String,
    kind: Kind,
    /// values set on the node, for inputs that aren't connected
    literals: Values,
    /// (input, from node, from output)
    connections: Vec<(Arc<str>, usize, Arc<str>)>,
    /// type and values set on the node, part of the memo key
    signature: String,
    realtime: bool,
    /// connections whose type doesn't fit the input they go into, the node fails without running
    bad_inputs: BadInputs,
    /// inputs that take any number of connections, they get them as one list
    multi: Vec<Arc<str>>,
}

struct ZonePlan {
    input: usize,
    output: usize,
    /// the nodes inside, in run order
    body: Vec<usize>,
    /// outputs of nodes outside that the zone reads, their values are part of its memo key
    reads: Vec<(usize, Arc<str>)>,
    signature: String,
    wants_previous: bool,
    wants_next: bool,
}

/// one graph, ready to run: node order, where each input comes from, and the zones
struct Plan {
    nodes: Vec<PlanNode>,
    order: Vec<usize>,
    zones: Vec<ZonePlan>,
    /// zone index by its output node
    zone_by_output: HashMap<usize, usize>,
    /// for the whole graph and for each zone's inside: the nodes that only run with a zone in it
    owned: HashMap<Option<usize>, HashSet<usize>>,
    group_output: Option<usize>,
    /// by node: the innermost zone it's inside
    zone_of: Vec<Option<usize>>,
}

impl Plan {
    /// `scope` is the group whose graph this is, `None` for the root graph
    fn new(ctx: &RunCtx, graph: &Graph, scope: Option<&GroupDef>) -> Self {
        let index: HashMap<&str, usize> = graph.nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
        // the sockets of every node, to check the types of its connections
        let specs = Specs {
            specs: ctx.specs,
            groups: ctx.groups,
            scope,
        };
        let sockets: Vec<Option<Cow<NodeSpec>>> = graph.nodes.iter().map(|node| socket_spec(&specs, node)).collect();

        let nodes: Vec<PlanNode> = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let node_type = node.resolved_node_type();
                let spec = find_spec(ctx.specs, node_type);
                let kind = match (node_type, spec) {
                    (_, None) => Kind::Broken(format!("unknown node type '{}'", node_type)),
                    (GROUP_INPUT, _) => Kind::GroupInput,
                    (GROUP_OUTPUT, _) => Kind::GroupOutput,
                    (GROUP, _) => Kind::Group(node.data.get("group_id").and_then(|v| v.as_str()).unwrap_or("").to_string()),
                    (FOR_EACH_INPUT | FOR_EACH_OUTPUT, _) => Kind::ZoneEnd,
                    _ => match ctx.registry.get(node_type) {
                        Some(func) => Kind::Executor(*func),
                        None => Kind::Broken(format!("no executor for node type '{}'", node_type)),
                    },
                };
                // note: inputs that aren't an object (a broken save file) are ignored
                let mut literals: Values = node.inputs().map(|inputs| inputs.iter().map(|(k, v)| (Arc::from(k.as_str()), ctx.memo.borrow_mut().literal(v))).collect()).unwrap_or_default();
                // inputs that aren't set get their default, a connection still replaces it
                for handle in spec.map_or(&[][..], |s| &s.handles.inputs[..]) {
                    if let Some(default) = handle.default.as_ref().filter(|_| !literals.iter().any(|(k, _)| **k == *handle.id)) {
                        literals.push((Arc::from(handle.id.as_str()), ctx.memo.borrow_mut().literal(default)));
                    }
                }
                let connections = graph.edges.iter().filter(|e| e.to_node() == node.id).filter_map(|e| Some((Arc::from(e.to_input()), *index.get(e.from_node())?, Arc::from(e.from_output())))).collect();
                let group_id = node.data.get("group_id").map(|v| v.to_string()).unwrap_or_default();
                let set = node.inputs().map(|i| Value::Object(i.clone()).to_string()).unwrap_or_default();
                let bad_inputs = match (&kind, &sockets[i]) {
                    (Kind::Broken(_), _) | (_, None) => BadInputs::new(),
                    (_, Some(to)) => graph.edges.iter().filter(|e| e.to_node() == node.id).filter_map(|e| Some((Arc::from(e.to_input()), connection_error(sockets[*index.get(e.from_node())?].as_deref()?, e.from_output(), to, e.to_input())?))).collect(),
                };
                let multi = sockets[i].as_deref().map(|s| s.handles.inputs.iter().filter(|h| h.multi).map(|h| Arc::from(h.id.as_str())).collect()).unwrap_or_default();
                // its own label, or what its group or type is called
                let label = node.label().map(str::to_string).or_else(|| node.data.get("group_id").and_then(Value::as_str).and_then(|g| ctx.groups.get(g)).map(|g| g.name.clone())).or_else(|| spec.map(|s| s.name.clone())).unwrap_or_else(|| node_type.to_string());
                PlanNode {
                    id: node.id.clone(),
                    node_type: node_type.to_string(),
                    label,
                    kind,
                    literals,
                    connections,
                    signature: format!("{}|{}|{}", node_type, group_id, set),
                    realtime: spec.map_or(true, |s| s.realtime),
                    bad_inputs,
                    multi,
                }
            })
            .collect();

        let order: Vec<usize> = graph.topo_order().iter().filter_map(|id| index.get(id.as_str()).copied()).collect();
        let mut plan = Plan {
            group_output: nodes.iter().position(|n| matches!(n.kind, Kind::GroupOutput)),
            nodes,
            order,
            zones: Vec::new(),
            zone_by_output: HashMap::new(),
            owned: HashMap::new(),
            zone_of: Vec::new(),
        };
        plan.find_zones(graph, &index);
        // a node in nested zones is in the inner one, it has the smaller inside
        plan.zone_of = vec![None; plan.nodes.len()];
        for (z, zone) in plan.zones.iter().enumerate() {
            for &node in &zone.body {
                if plan.zone_of[node].is_none_or(|other| plan.zones[other].body.len() > zone.body.len()) {
                    plan.zone_of[node] = Some(z);
                }
            }
        }
        // for the whole graph and each zone's inside, the zones in it own their input and inside
        let orders: Vec<(Option<usize>, Vec<usize>)> = std::iter::once((None, plan.order.clone())).chain(plan.zones.iter().enumerate().map(|(z, zone)| (Some(z), zone.body.clone()))).collect();
        for (key, order) in orders {
            let owned = order.iter().filter_map(|i| plan.zone_by_output.get(i)).flat_map(|z| std::iter::once(plan.zones[*z].input).chain(plan.zones[*z].body.iter().copied())).collect();
            plan.owned.insert(key, owned);
        }
        plan
    }

    /// pairs up the zone nodes and works out which nodes are inside each zone, a zone that's laid out wrong
    /// shows why on its nodes
    fn find_zones(&mut self, graph: &Graph, index: &HashMap<&str, usize>) {
        for (input, node) in graph.nodes.iter().enumerate() {
            if node.resolved_node_type() != FOR_EACH_INPUT || matches!(self.nodes[input].kind, Kind::Broken(_)) {
                continue;
            }
            // the paired output has to exist and point back
            let pair = node.data.get("zone").and_then(|v| v.as_str()).unwrap_or("");
            let paired = graph.node(pair).filter(|out| out.resolved_node_type() == FOR_EACH_OUTPUT && out.data.get("zone").and_then(|v| v.as_str()) == Some(&node.id));
            let Some(output_node) = paired else {
                self.nodes[input].kind = Kind::Broken(format!("'{}' has no matching for each output", node.id));
                continue;
            };
            let output = index[output_node.id.as_str()];

            // inside: depends on the zone input, and isn't the output or after it
            let after_input = descendants(graph, &node.id);
            let after_output = descendants(graph, &output_node.id);
            let inside: HashSet<&str> = after_input.iter().map(|s| s.as_str()).filter(|id| *id != output_node.id && !after_output.contains(*id)).collect();

            // a value from inside can only leave through the zone output
            if let Some(edge) = graph.edges.iter().find(|e| inside.contains(e.from_node()) && after_output.contains(e.to_node())) {
                let error = format!("'{}' uses '{}' from inside the loop, only the for each output can pass values out", edge.to_node(), edge.from_node());
                self.nodes[input].kind = Kind::Broken(error.clone());
                self.nodes[output].kind = Kind::Broken(error);
                continue;
            }

            let body: Vec<usize> = self.order.iter().copied().filter(|i| inside.contains(self.nodes[*i].id.as_str())).collect();
            let members: HashSet<usize> = body.iter().copied().chain([input, output]).collect();
            // what the inside, the input and the output read from outside the zone
            let mut reads: Vec<(usize, Arc<str>)> = members.iter().flat_map(|m| self.nodes[*m].connections.iter()).filter(|(_, from, _)| !members.contains(from)).map(|(_, from, output)| (*from, output.clone())).collect();
            reads.sort();
            reads.dedup();
            // the zone changes when a node inside or a connection inside does
            let mut signatures: Vec<&str> = members.iter().map(|m| self.nodes[*m].signature.as_str()).collect();
            signatures.sort();
            let mut edges: Vec<&str> = graph.edges.iter().filter(|e| inside.contains(e.to_node()) || e.to_node() == output_node.id).map(|e| e.id.as_str()).collect();
            edges.sort();
            let wants = |out: &str| graph.edges_from(&node.id, Some(out)).next().is_some();

            self.zone_by_output.insert(output, self.zones.len());
            self.zones.push(ZonePlan {
                input,
                output,
                body,
                reads,
                signature: format!("{}#{}", signatures.join(";"), edges.join(";")),
                wants_previous: wants("previous"),
                wants_next: wants("next"),
            });
        }
    }
}

/// every node data can reach from `start`, not including `start`
fn descendants(graph: &Graph, start: &str) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut stack = vec![start.to_string()];
    while let Some(id) = stack.pop() {
        for edge in graph.edges_from(&id, None) {
            if seen.insert(edge.to_node().to_string()) {
                stack.push(edge.to_node().to_string());
            }
        }
    }
    seen
}

/// the sockets to check a node's connections against, `None` when they aren't known (an unknown node type, a group
/// node whose group is missing, a group input or output outside a group)
fn socket_spec<'s>(specs: &Specs<'s>, node: &RfNode) -> Option<Cow<'s, NodeSpec>> {
    match node.resolved_node_type() {
        GROUP if !node.data.get("group_id").and_then(|v| v.as_str()).is_some_and(|g| specs.groups.contains_key(g)) => None,
        GROUP_INPUT | GROUP_OUTPUT if specs.scope.is_none() => None,
        _ => specs.for_node(node),
    }
}

/// an input's handle and type, a dynamic input (`object_maps_0`) has its `Dyn<T>` handle and `T`
fn input_handle<'s>(spec: &'s NodeSpec, input: &str) -> Option<(&'s HandleSpec, &'s str)> {
    if let Some(handle) = spec.input(input) {
        return Some((handle, handle.data_type.as_str()));
    }
    spec.handles.inputs.iter().find(|h| dyn_index(&h.id, input).is_some()).and_then(|h| Some((h, dyn_inner(h)?)))
}

/// an output's name and type, any other output of a node with a `Dyn<T>` output is one of its dynamic outputs of type `T`
fn output_handle<'s>(spec: &'s NodeSpec, output: &'s str) -> Option<(&'s str, &'s str)> {
    if let Some(handle) = spec.output(output) {
        return Some((handle.name.as_str(), handle.data_type.as_str()));
    }
    spec.handles.outputs.iter().find_map(dyn_inner).map(|inner| (output, inner))
}

/// what's wrong with a connection from `output` on `from` to `input` on `to`, `None` if its types fit.
/// a socket that isn't there is left alone, the input gets nothing and the node uses its default
fn connection_error(from: &NodeSpec, output: &str, to: &NodeSpec, input: &str) -> Option<String> {
    let (in_handle, in_ty) = input_handle(to, input)?;
    let (out_name, out_ty) = output_handle(from, output)?;
    if compatible(out_ty, in_ty) {
        return None;
    }
    Some(format!("{} expects {}, but {} › {} gives {}", in_handle.name, in_ty, from.name, out_name, out_ty))
}

// MARK: - Run

/// how running an executor went: what it gave and the inputs that had the wrong type, or it's missing a required input.
/// `true` when it came from the memo
enum Ran {
    Done(Result<Outputs, String>, BadInputs, bool),
    Waiting(bool),
}

/// what a node gave in this run
#[derive(Clone)]
enum Slot {
    Done(Outputs),
    Failed,
}

/// the node values of one graph run, or of one zone item, which falls back to the enclosing run for nodes outside the zone
struct Frame<'p> {
    slots: Vec<Option<Slot>>,
    parent: Option<&'p Frame<'p>>,
}

impl<'p> Frame<'p> {
    fn new(size: usize, parent: Option<&'p Frame<'p>>) -> Self {
        Self {
            slots: vec![None; size],
            parent,
        }
    }

    fn get(&self, index: usize) -> Option<&Slot> {
        self.slots[index].as_ref().or_else(|| self.parent?.get(index))
    }

    fn set(&mut self, index: usize, outcome: &Result<Outputs, String>) {
        self.slots[index] = Some(match outcome {
            Ok(outputs) => Slot::Done(outputs.clone()),
            Err(_) => Slot::Failed,
        });
    }
}

/// where a run records what nodes did, `None` inside zone items after the first
type Sink<'r> = Option<&'r mut Vec<Entry>>;

/// runs one graph (the root, or a group's graph)
struct Runner<'a, 'c> {
    ctx: &'a RunCtx<'c>,
    plan: Rc<Plan>,
    /// path of the group node this graph runs for, with a trailing separator. empty for the root graph
    prefix: String,
    /// what the `group_input` nodes output
    group_inputs: &'a Outputs,
    /// how each node went, by node
    times: RefCell<Vec<Timing>>,
}

/// how one node went while its graph ran, added up over a zone's items
#[derive(Default, Clone)]
struct Timing {
    elapsed: Duration,
    runs: u32,
    hits: u32,
    status: Option<Status>,
    error: Option<String>,
    own_error: bool,
    /// a zone's items
    items: Option<u32>,
}

/// runs a graph and returns what its `group_output` node got (nothing if it has none).
/// `stack` is the group ids being run, so a group can't run itself.
/// errors with the first failed node, the rest of the graph still runs and is recorded
fn run_graph(ctx: &RunCtx, graph: &Graph, group_id: Option<&str>, prefix: &str, group_inputs: &Outputs, stack: &mut Vec<String>, record: &mut Sink) -> Result<Outputs, String> {
    let plan = ctx.plan(graph, group_id);
    let runner = Runner {
        ctx,
        times: RefCell::new(vec![Timing::default(); plan.nodes.len()]),
        plan,
        prefix: prefix.to_string(),
        group_inputs,
    };
    let mut frame = Frame::new(runner.plan.nodes.len(), None);
    let first_error = runner.run_nodes(None, &mut frame, stack, record, false);
    runner.add_timings();

    // the group's outputs are whatever its output node got
    let outputs = match runner.plan.group_output.and_then(|i| frame.slots[i].clone()) {
        Some(Slot::Done(outputs)) => outputs,
        _ => Outputs::new(),
    };
    match first_error {
        Some(error) => Err(error),
        None => Ok(outputs),
    }
}

impl<'a, 'c> Runner<'a, 'c> {
    fn path(&self, index: usize) -> String {
        format!("{}{}", self.prefix, self.plan.nodes[index].id)
    }

    /// a node came up: how long since `start`, whether it came from the memo, and how it went. a failure in any of a
    /// zone's items is how it went
    fn time(&self, index: usize, start: Instant, cached: bool, status: Status, error: Option<&String>, own_error: bool) {
        let elapsed = start.elapsed();
        let timing = &mut self.times.borrow_mut()[index];
        timing.elapsed += elapsed;
        if cached {
            timing.hits += 1;
        } else {
            timing.runs += 1;
        }
        if timing.status != Some(Status::Failed) {
            timing.status = Some(status);
            timing.error = error.cloned();
            timing.own_error = own_error;
        }
    }

    /// the timings of the nodes that came up go to the run, by path
    fn add_timings(&self) {
        let group = self.prefix.strip_suffix(PATH_SEP).map(str::to_string);
        for (index, timing) in self.times.take().into_iter().enumerate() {
            let Some(status) = timing.status else {
                continue;
            };
            let node = &self.plan.nodes[index];
            let zone = self.plan.zone_of[index].map(|z| self.path(self.plan.zones[z].output));
            self.ctx.add_timing(NodeTiming {
                node: self.path(index),
                node_type: node.node_type.clone(),
                label: node.label.clone(),
                cached: timing.runs == 0,
                ms: timing.elapsed.as_secs_f64() * 1000.0,
                status,
                error: timing.error,
                group: group.clone(),
                iterations: timing.items.or((zone.is_some() && timing.runs > 0).then_some(timing.runs)),
                zone,
                own_error: timing.own_error,
            });
        }
    }

    /// runs `order` (already sorted), zones run when their output node comes up.
    /// `in_item` is set inside a zone item, where nothing is memoized (every item has the same paths).
    /// returns the first error, inside an item it stops there instead of running the other branches
    fn run_nodes(&self, zone: Option<usize>, frame: &mut Frame, stack: &mut Vec<String>, record: &mut Sink, in_item: bool) -> Option<String> {
        // the whole graph, or a zone's inside. the zones whose output is in it run here, their insides run with them
        let order = zone.map_or(&self.plan.order, |z| &self.plan.zones[z].body);
        let owned = &self.plan.owned[&zone];

        let mut first_error = None;
        for &index in order.iter() {
            if owned.contains(&index) {
                continue;
            }
            let outcome = match self.plan.zone_by_output.get(&index) {
                Some(zone) => self.run_zone(*zone, frame, stack, record, in_item),
                None => self.run_node(index, frame, stack, record, in_item),
            };
            if let Err(error) = outcome {
                first_error.get_or_insert(error);
                if in_item {
                    break;
                }
            }
        }
        first_error
    }

    /// the inputs of a node: the values set on it, replaced by its connections. a multi input gets all of its
    /// connections as one list, in the order they were made.
    /// `None` if something it's connected to didn't run or failed, then it doesn't run either
    fn gather(&self, index: usize, frame: &Frame) -> Option<Values> {
        let node = &self.plan.nodes[index];
        let mut values = node.literals.clone();
        let mut lists: Vec<(Arc<str>, Vec<Val>)> = Vec::new();
        for (input, from, output) in &node.connections {
            // only the failed node shows the error
            let Slot::Done(outputs) = frame.get(*from)? else {
                return None;
            };
            let value = outputs.get(output).cloned().unwrap_or_else(|| Val::json(Value::Null));

            // a multi input collects its connections, a list connected to it is flattened in and nothing adds nothing
            if node.multi.contains(input) {
                let at = lists.iter().position(|(k, _)| k == input).unwrap_or_else(|| {
                    lists.push((input.clone(), Vec::new()));
                    lists.len() - 1
                });
                match value.items() {
                    Some(items) => lists[at].1.extend(items),
                    None if value.is_null() => {}
                    None => lists[at].1.push(value),
                }
                continue;
            }
            set_value(&mut values, input, value);
        }
        for (input, items) in lists {
            set_value(&mut values, &input, Val::list(items));
        }
        Some(values)
    }

    /// stores what a node gave and records it, errors with the path when it failed
    fn finish(&self, index: usize, outcome: Result<Outputs, String>, inputs: Values, bad_inputs: BadInputs, frame: &mut Frame, record: &mut Sink) -> Result<(), String> {
        frame.set(index, &outcome);
        // the path is only made when something needs it, most nodes in a loop don't
        let error = outcome.as_ref().err().map(|message| {
            let path = self.path(index);
            eprintln!("ERROR: node '{}' failed: {}", path, message);
            format!("{}: {}", path, message)
        });
        if let Some(record) = record {
            record.push(Entry {
                path: self.path(index),
                outputs: Some(outcome),
                inputs,
                bad_inputs,
            });
        }
        error.map_or(Ok(()), Err)
    }

    /// fails a node with connections of the wrong type without running it, even if what it's connected to didn't run
    fn reject(&self, index: usize, start: Instant, frame: &mut Frame, record: &mut Sink) -> Result<(), String> {
        let node = &self.plan.nodes[index];
        let inputs = self.gather(index, frame).unwrap_or_else(|| node.literals.clone());
        let message = node.bad_inputs.iter().map(|(_, m)| m.as_str()).collect::<Vec<_>>().join("; ");
        self.time(index, start, false, Status::Failed, Some(&message), true);
        self.finish(index, Err(message), inputs, node.bad_inputs.clone(), frame, record)
    }

    /// runs one node that isn't part of a zone
    fn run_node(&self, index: usize, frame: &mut Frame, stack: &mut Vec<String>, record: &mut Sink, in_item: bool) -> Result<(), String> {
        let node = &self.plan.nodes[index];
        let start = Instant::now();
        if !node.bad_inputs.is_empty() {
            return self.reject(index, start, frame, record);
        }
        let Some(inputs) = self.gather(index, frame) else {
            return Ok(());
        };
        // not run in a realtime run, only what it got is shown
        if self.ctx.realtime && !node.realtime {
            let path = self.path(index);
            let fresh = self.ctx.memo.borrow_mut().still_written(&path, &inputs);
            self.ctx.writes.borrow_mut().push((
                path,
                Write::Skipped {
                    fresh,
                },
            ));
            if let Some(record) = record {
                record.push(Entry {
                    path: self.path(index),
                    outputs: None,
                    inputs,
                    bad_inputs: BadInputs::new(),
                });
            }
            self.time(index, start, false, Status::Skipped, None, false);
            return Ok(());
        }

        let (outcome, bad_inputs, cached) = match &node.kind {
            Kind::GroupInput => (Ok(self.group_inputs.clone()), BadInputs::new(), false),
            Kind::GroupOutput => (Ok(values_to_outputs(&inputs)), BadInputs::new(), false),
            Kind::Group(group_id) => (self.run_group(index, group_id, &inputs, stack, record), BadInputs::new(), false),
            Kind::ZoneEnd => (Err(format!("'{}' isn't part of a for each zone", node.id)), BadInputs::new(), false),
            Kind::Broken(message) => (Err(message.clone()), BadInputs::new(), false),
            Kind::Executor(func) => match self.run_executor(index, *func, &inputs, in_item) {
                Ran::Done(outcome, bad_inputs, cached) => (outcome, bad_inputs, cached),
                Ran::Waiting(cached) => {
                    self.wait(index, inputs, frame, record);
                    self.time(index, start, cached, Status::Waiting, None, false);
                    return Ok(());
                }
            },
        };
        if !node.realtime {
            let path = self.path(index);
            self.ctx.memo.borrow_mut().set_written(&path, outcome.is_ok().then(|| inputs.clone()));
            self.ctx.writes.borrow_mut().push((path, Write::Ran(outcome.as_ref().err().cloned())));
        }
        // a group's error is the one inside it
        let status = if outcome.is_ok() {
            Status::Ok
        } else {
            Status::Failed
        };
        self.time(index, start, cached, status, outcome.as_ref().err(), !matches!(node.kind, Kind::Group(_)));
        self.finish(index, outcome, inputs, bad_inputs, frame, record)
    }

    /// a node missing a required input: it shows what it got, and the nodes after it don't run
    fn wait(&self, index: usize, inputs: Values, frame: &mut Frame, record: &mut Sink) {
        frame.slots[index] = Some(Slot::Failed);
        if let Some(record) = record {
            record.push(Entry {
                path: self.path(index),
                outputs: None,
                inputs,
                bad_inputs: BadInputs::new(),
            });
        }
    }

    /// runs an executor, or takes what it gave last time for the same inputs
    fn run_executor(&self, index: usize, func: NodeFunction, inputs: &Values, in_item: bool) -> Ran {
        let node = &self.plan.nodes[index];
        let path = self.path(index);
        // nothing connected: it brings in the outside world (a file, the scene), which can change without the graph changing.
        // not realtime: it writes to Blender, which should happen every time it's asked to
        let source = node.connections.is_empty();
        let reusable = !source && node.realtime;
        if !in_item && reusable {
            if let Some(memo) = self.ctx.memo.borrow_mut().hit(&path, &node.signature, inputs) {
                return match memo.waiting {
                    true => Ran::Waiting(true),
                    false => Ran::Done(memo.outputs.clone(), memo.bad_inputs.clone(), true),
                };
            }
        }

        // a panic that slipped through is turned into an error too so it can't take the app down
        let call = Inputs::new(inputs.clone(), &self.ctx.conversions);
        let mut outcome = catch_unwind(AssertUnwindSafe(|| func(&call))).unwrap_or_else(|panic| Err(format!("crashed: {}", panic_message(&panic))));
        let bad_inputs: BadInputs = match &outcome {
            Err(message) => call.wrong_type().into_iter().map(|input| (input, message.clone())).collect(),
            Ok(_) => BadInputs::new(),
        };
        let waiting = outcome.is_err() && call.missing() && bad_inputs.is_empty();

        if !in_item {
            let mut memo = self.ctx.memo.borrow_mut();
            // a node bringing in the outside world keeps its old values when they come out the same, so the nodes after it don't run again
            if let (true, Ok(outputs), Some(Ok(old))) = (source, &outcome, memo.nodes.get(&path).map(|m| &m.outputs)) {
                let unchanged = old.iter().count() == outputs.iter().count() && outputs.iter().all(|(k, v)| old.get(k).is_some_and(|o| o.to_json() == v.to_json()));
                if unchanged {
                    outcome = Ok(old.clone());
                }
            }
            memo.store(
                &path,
                NodeMemo {
                    signature: node.signature.clone(),
                    key: inputs.clone(),
                    outputs: outcome.clone(),
                    bad_inputs: bad_inputs.clone(),
                    waiting,
                    record: Vec::new(),
                },
            );
        }
        match waiting {
            true => Ran::Waiting(false),
            false => Ran::Done(outcome, bad_inputs, false),
        }
    }

    /// runs a group node's graph with the group node's inputs
    fn run_group(&self, index: usize, group_id: &str, inputs: &Values, stack: &mut Vec<String>, record: &mut Sink) -> Result<Outputs, String> {
        let def = self.ctx.groups.get(group_id).ok_or_else(|| format!("node group '{}' doesn't exist", group_id))?;
        if stack.iter().any(|g| g == group_id) {
            return Err(format!("node group '{}' contains itself", def.name));
        }

        // a group's inside is only recorded while it's open in the editor (or a group inside it is)
        let path = self.path(index);
        let open = self.ctx.inspect == path || self.ctx.inspect.starts_with(&format!("{}{}", path, PATH_SEP));

        stack.push(group_id.to_string());
        let prefix = format!("{}{}", path, PATH_SEP);
        let mut inner: Sink = if open {
            record.as_deref_mut()
        } else {
            None
        };
        let outcome = run_graph(self.ctx, &def.graph, Some(group_id), &prefix, &values_to_outputs(inputs), stack, &mut inner);
        stack.pop();
        // the inner node that failed has the full message, the group says where to look
        outcome.map_err(|e| format!("failed inside the group at {}", e))
    }

    /// runs a for each zone: the nodes inside once per item, collecting what reaches the zone output
    fn run_zone(&self, zone_index: usize, frame: &mut Frame, stack: &mut Vec<String>, record: &mut Sink, in_item: bool) -> Result<(), String> {
        let zone = &self.plan.zones[zone_index];
        let start = Instant::now();
        // a connection of the wrong type into either end stops the whole zone
        for end in [zone.input, zone.output] {
            if !self.plan.nodes[end].bad_inputs.is_empty() {
                frame.slots[zone.input] = Some(Slot::Failed);
                frame.slots[zone.output] = Some(Slot::Failed);
                return self.reject(end, start, frame, record);
            }
        }
        let Some(inputs) = self.gather(zone.input, frame) else {
            return Ok(());
        };

        // the zone is memoized as a whole: same nodes inside, same items and same values read from outside
        let mut key = inputs.clone();
        for (from, output) in &zone.reads {
            match frame.get(*from) {
                Some(Slot::Done(outputs)) => key.push((output.clone(), outputs.get(output).cloned().unwrap_or_else(|| Val::json(Value::Null)))),
                _ => return Ok(()),
            }
        }
        let path = self.path(zone.output);
        let hit = if in_item {
            None
        } else {
            self.ctx.memo.borrow_mut().hit(&path, &zone.signature, &key).map(|m| (m.outputs.clone(), m.record.clone()))
        };

        let cached = hit.is_some();
        let mut items = None;
        let (outputs, zone_record) = match hit {
            Some(hit) => hit,
            None => {
                let (outputs, zone_record, ran) = self.run_items(zone_index, &inputs, frame, stack);
                items = ran;
                if !in_item {
                    self.ctx.memo.borrow_mut().store(
                        &path,
                        NodeMemo {
                            signature: zone.signature.clone(),
                            key,
                            outputs: outputs.clone(),
                            bad_inputs: BadInputs::new(),
                            waiting: false,
                            record: zone_record.clone(),
                        },
                    );
                }
                (outputs, zone_record)
            }
        };

        frame.slots[zone.input] = Some(Slot::Done(Outputs::new()));
        frame.set(zone.output, &outputs);
        if let Some(record) = record {
            record.extend(zone_record);
        }

        // the zone's time is its output's. one from the memo didn't run anything inside, its nodes came from it too.
        // the error is the zone's own if it failed before its items ran
        let status = if outputs.is_ok() {
            Status::Ok
        } else {
            Status::Failed
        };
        self.time(zone.output, start, cached, status, outputs.as_ref().err(), !cached && items.is_none());
        // a zone inside another one adds up its items over the outer one's
        if let Some(items) = items {
            let timing = &mut self.times.borrow_mut()[zone.output];
            timing.items = Some(timing.items.unwrap_or(0) + items);
        }
        if cached && outputs.is_ok() {
            let mut times = self.times.borrow_mut();
            for &node in &zone.body {
                times[node].hits += 1;
                times[node].status.get_or_insert(Status::Ok);
            }
        }
        outputs.map(|_| ()).map_err(|message| {
            eprintln!("ERROR: node '{}' failed: {}", path, message);
            format!("{}: {}", path, message)
        })
    }

    /// the zone's items, each run through the nodes inside. gives the zone output's outputs, the record of the first
    /// item (the zone output's entry last), and how many items ran (none when it failed before the first)
    fn run_items(&self, zone_index: usize, inputs: &Values, frame: &Frame, stack: &mut Vec<String>) -> (Result<Outputs, String>, Vec<Entry>, Option<u32>) {
        let zone = &self.plan.zones[zone_index];
        let mut zone_record: Vec<Entry> = Vec::new();
        let output_entry = |outputs: &Result<Outputs, String>, result: Values| Entry {
            path: self.path(zone.output),
            outputs: Some(outputs.clone()),
            inputs: result,
            bad_inputs: BadInputs::new(),
        };
        let fail = |message: String, mut zone_record: Vec<Entry>, ran: Option<u32>| {
            let outputs = Err(message);
            zone_record.push(output_entry(&outputs, Values::new()));
            (outputs, zone_record, ran)
        };
        if let Kind::Broken(message) = &self.plan.nodes[zone.input].kind {
            return fail(message.clone(), zone_record, None);
        }

        // no items is an empty loop, anything else has to be a list
        let items = match inputs.iter().find(|(k, _)| &**k == "items").map(|(_, v)| v) {
            None => Vec::new(),
            Some(value) if value.is_null() => Vec::new(),
            Some(value) => match value.items() {
                Some(items) => items,
                None => return fail(format!("for each needs a list of items, got {}", json_kind(&value.to_json())), zone_record, None),
            },
        };
        let count = Val::new(items.len() as u32);
        let null = Val::json(Value::Null);

        let mut collected = Vec::with_capacity(items.len());
        let mut first_result = Values::new();
        // one frame for every item, the zone's nodes are cleared between items
        let mut item_frame = Frame::new(self.plan.nodes.len(), Some(frame));
        for (index, item) in items.iter().enumerate() {
            item_frame.slots[zone.input] = None;
            for &node in &zone.body {
                item_frame.slots[node] = None;
            }
            let mut item_values = Outputs::new();
            item_values.set_val("element", item.clone());
            item_values.set("index", index as u32);
            item_values.set_val("count", count.clone());
            if zone.wants_previous {
                item_values.set_val("previous", index.checked_sub(1).map_or_else(|| null.clone(), |i| items[i].clone()));
            }
            if zone.wants_next {
                item_values.set_val("next", items.get(index + 1).cloned().unwrap_or_else(|| null.clone()));
            }

            // only the first item is recorded for the UI
            let mut first = Vec::new();
            let mut sink: Sink = if index == 0 {
                Some(&mut first)
            } else {
                None
            };
            let _ = self.finish(zone.input, Ok(item_values), inputs.clone(), BadInputs::new(), &mut item_frame, &mut sink);
            let error = self.run_nodes(Some(zone_index), &mut item_frame, stack, &mut sink, true);
            zone_record.extend(first);
            let ran = Some(index as u32 + 1);
            if let Some(error) = error {
                return fail(format!("item {}: {}", index, error), zone_record, ran);
            }

            // what reached the zone output this item, nothing if the node feeding it didn't run
            let Some(result) = self.gather(zone.output, &item_frame) else {
                return (Ok(Outputs::new()), zone_record, ran);
            };
            collected.push(result.iter().find(|(k, _)| &**k == "result").map_or_else(|| null.clone(), |(_, v)| v.clone()));
            if index == 0 {
                first_result = result;
            }
        }

        let mut outputs = Outputs::new();
        outputs.set_val("results", Val::list(collected));
        let outputs = Ok(outputs);
        zone_record.push(output_entry(&outputs, first_result));
        (outputs, zone_record, Some(items.len() as u32))
    }
}

/// sets an input's value, replacing the one set on the node
fn set_value(values: &mut Values, input: &Arc<str>, value: Val) {
    match values.iter_mut().find(|(k, _)| k == input) {
        Some(slot) => slot.1 = value,
        None => values.push((input.clone(), value)),
    }
}

/// a node's inputs passed on as outputs (group input and output)
fn values_to_outputs(values: &Values) -> Outputs {
    let mut outputs = Outputs::new();
    for (k, v) in values {
        outputs.set_val(k, v.clone());
    }
    outputs
}

/// "a string", "a number", ... for error messages
fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "nothing",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "a list",
        Value::Object(_) => "an object",
    }
}

/// the message of a caught panic, it can be a &str or a String
pub fn panic_message(panic: &Box<dyn std::any::Any + Send>) -> String {
    panic.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| panic.downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown panic".to_string())
}

/// path of a group node running `group_id`, searching the root graph first, then inside its groups.
/// the values recorded for the group's insides are under this path
pub fn instance_path(graph: &Graph, groups: &BTreeMap<String, GroupDef>, group_id: &str) -> Option<String> {
    fn search(graph: &Graph, groups: &BTreeMap<String, GroupDef>, group_id: &str, seen: &mut Vec<String>) -> Option<String> {
        let group_nodes: Vec<_> = graph.nodes.iter().filter(|n| n.resolved_node_type() == GROUP).filter_map(|n| Some((n, n.data.get("group_id")?.as_str()?))).collect();
        if let Some((node, _)) = group_nodes.iter().find(|(_, g)| *g == group_id) {
            return Some(node.id.clone());
        }
        for (node, inner_id) in group_nodes {
            // a group can't contain itself, but a broken project could say it does
            if seen.iter().any(|s| s == inner_id) {
                continue;
            }
            let Some(inner) = groups.get(inner_id) else {
                continue;
            };
            seen.push(inner_id.to_string());
            if let Some(path) = search(&inner.graph, groups, group_id, seen) {
                return Some(format!("{}{}{}", node.id, PATH_SEP, path));
            }
            seen.pop();
        }
        None
    }
    search(graph, groups, group_id, &mut Vec::new())
}

/// the values recorded inside the group node at `path`, keyed by the inner node ids
pub fn scoped_values(values: &HashMap<String, Value>, path: &str) -> HashMap<String, Value> {
    let prefix = format!("{}{}", path, PATH_SEP);
    values.iter().filter_map(|(k, v)| k.strip_prefix(&prefix).filter(|rest| !rest.contains(PATH_SEP)).map(|rest| (rest.to_string(), v.clone()))).collect()
}

/// paths of the nodes writing to Blender (not realtime) in a graph and in the groups its group nodes run, at any depth
pub fn write_paths(graph: &Graph, groups: &BTreeMap<String, GroupDef>, specs: &[NodeSpec]) -> Vec<String> {
    fn search(graph: &Graph, groups: &BTreeMap<String, GroupDef>, specs: &[NodeSpec], prefix: &str, seen: &mut Vec<String>, paths: &mut Vec<String>) {
        for node in &graph.nodes {
            let node_type = node.resolved_node_type();
            if node_type == GROUP {
                // a group can't contain itself, but a broken project could say it does
                let Some(inner_id) = node.data.get("group_id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(inner) = groups.get(inner_id).filter(|_| !seen.iter().any(|s| s == inner_id)) else {
                    continue;
                };
                seen.push(inner_id.to_string());
                search(&inner.graph, groups, specs, &format!("{}{}{}", prefix, node.id, PATH_SEP), seen, paths);
                seen.pop();
            } else if find_spec(specs, node_type).is_some_and(|spec| !spec.realtime) {
                paths.push(format!("{}{}", prefix, node.id));
            }
        }
    }
    let mut paths = Vec::new();
    search(graph, groups, specs, "", &mut Vec::new(), &mut paths);
    paths
}
