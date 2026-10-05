// undo history for the project graph. a change is kept as the records it touched (nodes, edges, group definitions)
// the way they were before and after, found by diffing the project, so no edit needs undo code of its own.
// undo puts every record back the way it was before, redo the way it was after

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// how many entries are kept by default, the oldest go first
pub const DEFAULT_LIMIT: usize = 100;

/// node fields only the UI cares about, never recorded and left as they are when a record is put back.
/// selection is recorded, selecting is an undo step like in blender
const UI_NODE_KEYS: &[&str] = &["dragging", "measured", "resizing"];
/// edge fields only the UI cares about
const UI_EDGE_KEYS: &[&str] = &[];
/// node and edge fields that change how the graph looks but not what it computes
const LAYOUT_KEYS: &[&str] = &["position", "selected", "width", "height"];
/// fields of a graph that aren't part of its own record: its nodes, edges and groups are records of their own,
/// and the viewport (where the graph is looked at) is never recorded
const GRAPH_KEYS: &[&str] = &["nodes", "edges", "groups", "viewport"];

// MARK: - Records

/// the kinds of records, in the order they're put back (a group before the nodes inside it)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Root,
    Group,
    Node,
    Edge,
}

/// names one record: its kind, the group it's inside (`None` at the top level) and its id
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    kind: Kind,
    scope: Option<String>,
    id: String,
}

impl Key {
    /// the list a node or edge record lives in
    fn list(&self) -> &'static str {
        if self.kind == Kind::Edge {
            "edges"
        } else {
            "nodes"
        }
    }

    /// the fields kept out of this record
    fn ui_keys(&self) -> &'static [&'static str] {
        match self.kind {
            Kind::Node => UI_NODE_KEYS,
            Kind::Edge => UI_EDGE_KEYS,
            Kind::Root | Kind::Group => GRAPH_KEYS,
        }
    }
}

/// `object` without `keys`. not selected and no `selected` are the same
fn without(object: &Map<String, Value>, keys: &[&str]) -> Value {
    Value::Object(object.iter().filter(|(k, v)| !keys.contains(&k.as_str()) && !(k.as_str() == "selected" && **v == Value::Bool(false))).map(|(k, v)| (k.clone(), v.clone())).collect())
}

/// a node or edge without what only changes how it looks, its label too
fn without_layout(object: &Map<String, Value>) -> Value {
    let mut value = without(object, LAYOUT_KEYS);
    if let Some(data) = value.get_mut("data").and_then(|d| d.as_object_mut()) {
        data.remove("label");
    }
    value
}

/// every record in a project with its place in its list
fn records(project: &Map<String, Value>) -> BTreeMap<Key, (Value, usize)> {
    let mut out = BTreeMap::new();
    let key = |kind, scope: Option<&String>, id: &str| Key {
        kind,
        scope: scope.cloned(),
        id: id.to_string(),
    };

    // the nodes and edges of one graph, ones without an id can't be told apart and are left out
    let graph_records = |graph: &Map<String, Value>, scope: Option<&String>, out: &mut BTreeMap<Key, (Value, usize)>| {
        for (kind, list) in [(Kind::Node, "nodes"), (Kind::Edge, "edges")] {
            let items = graph.get(list).and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
            for (at, item) in items.iter().enumerate() {
                let (Some(id), Some(item)) = (item.get("id").and_then(Value::as_str), item.as_object()) else {
                    continue;
                };
                let key = key(kind, scope, id);
                let value = without(item, key.ui_keys());
                out.insert(key, (value, at));
            }
        }
    };

    // the top level, then each group definition and what's inside it
    out.insert(key(Kind::Root, None, ""), (without(project, GRAPH_KEYS), 0));
    graph_records(project, None, &mut out);
    for (id, def) in project.get("groups").and_then(Value::as_object).into_iter().flatten() {
        let Some(def) = def.as_object() else {
            continue;
        };
        out.insert(key(Kind::Group, None, id), (without(def, GRAPH_KEYS), 0));
        graph_records(def, Some(id), &mut out);
    }
    out
}

/// the graph a record lives in: the top level, or a group definition
fn graph_mut<'a>(project: &'a mut Map<String, Value>, scope: &Option<String>) -> Option<&'a mut Map<String, Value>> {
    match scope {
        None => Some(project),
        Some(id) => project.get_mut("groups")?.as_object_mut()?.get_mut(id)?.as_object_mut(),
    }
}

/// writes a record into the project, replacing the one with the same key or inserting it at `at`
fn put(project: &mut Map<String, Value>, key: &Key, value: &Value, at: usize) {
    let Some(fields) = value.as_object() else {
        return;
    };
    match key.kind {
        // replace the graph's own fields, its nodes, edges, groups and viewport stay
        Kind::Root | Kind::Group => {
            let target = if key.kind == Kind::Root {
                project
            } else {
                let groups = project.entry("groups").or_insert_with(|| Value::Object(Map::new()));
                let Some(groups) = groups.as_object_mut() else {
                    return;
                };
                // a group being put back starts out empty, its nodes and edges are records of their own
                let def = groups.entry(key.id.clone()).or_insert_with(|| serde_json::json!({ "nodes": [], "edges": [] }));
                let Some(def) = def.as_object_mut() else {
                    return;
                };
                def
            };
            target.retain(|k, _| GRAPH_KEYS.contains(&k.as_str()));
            target.extend(fields.clone());
        }
        // replace the node or edge in place keeping its UI only fields, or insert it where it was
        Kind::Node | Kind::Edge => {
            let Some(graph) = graph_mut(project, &key.scope) else {
                return;
            };
            let list = graph.entry(key.list()).or_insert_with(|| Value::Array(Vec::new()));
            let Some(list) = list.as_array_mut() else {
                return;
            };
            let mut item = fields.clone();
            match list.iter_mut().find(|item| item.get("id").and_then(Value::as_str) == Some(&key.id)) {
                Some(existing) => {
                    for ui in key.ui_keys() {
                        if let Some(v) = existing.get(*ui) {
                            item.insert(ui.to_string(), v.clone());
                        }
                    }
                    *existing = Value::Object(item);
                }
                None => list.insert(at.min(list.len()), Value::Object(item)),
            }
        }
    }
}

/// removes a record from the project, a removed group takes its nodes and edges with it
fn remove(project: &mut Map<String, Value>, key: &Key) {
    match key.kind {
        // the top level is always there
        Kind::Root => {}
        Kind::Group => {
            if let Some(groups) = project.get_mut("groups").and_then(Value::as_object_mut) {
                groups.remove(&key.id);
                // a project without groups has no `groups` at all, like `Graph` saves it
                if groups.is_empty() {
                    project.remove("groups");
                }
            }
        }
        Kind::Node | Kind::Edge => {
            if let Some(list) = graph_mut(project, &key.scope).and_then(|g| g.get_mut(key.list())).and_then(Value::as_array_mut) {
                list.retain(|item| item.get("id").and_then(Value::as_str) != Some(&key.id));
            }
        }
    }
}

// MARK: - Diff

/// one record before and after a change, `None` where it didn't exist. `*_at` is its place in its list then
#[derive(Clone, Debug)]
struct Change {
    key: Key,
    before: Option<Value>,
    after: Option<Value>,
    before_at: usize,
    after_at: usize,
}

impl Change {
    /// the record after the change (`forward`) or before it, and its place in its list then
    fn target(&self, forward: bool) -> (&Option<Value>, usize) {
        if forward {
            (&self.after, self.after_at)
        } else {
            (&self.before, self.before_at)
        }
    }
}

/// a change to a project: every record it touched, before and after
#[derive(Clone, Debug, Default)]
pub struct Diff(Vec<Change>);

impl Diff {
    /// the records that differ between two projects. a record that only moved in its list isn't a change
    pub fn between(before: &HashMap<String, Value>, after: &HashMap<String, Value>) -> Self {
        let old = records(&before.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        let new = records(&after.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        let keys: BTreeSet<&Key> = old.keys().chain(new.keys()).collect();
        let changes = keys
            .into_iter()
            .filter_map(|key| {
                let (before, before_at) = old.get(key).map_or((None, 0), |(v, at)| (Some(v), *at));
                let (after, after_at) = new.get(key).map_or((None, 0), |(v, at)| (Some(v), *at));
                (before != after).then(|| Change {
                    key: key.clone(),
                    before: before.cloned(),
                    after: after.cloned(),
                    before_at,
                    after_at,
                })
            })
            .collect();
        Self(changes)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// this change followed by `next`, as one change. records that end up the way they started drop out
    pub fn then(&mut self, next: Diff) {
        for change in next.0 {
            match self.0.iter_mut().find(|c| c.key == change.key) {
                Some(existing) => {
                    existing.after = change.after;
                    existing.after_at = change.after_at;
                }
                None => self.0.push(change),
            }
        }
        self.0.retain(|c| c.before != c.after);
    }

    /// true if the change can change what the graph computes, not only where things are or what's selected
    pub fn affects_output(&self) -> bool {
        let layout_only = |c: &Change| match (&c.before, &c.after) {
            (Some(Value::Object(before)), Some(Value::Object(after))) if matches!(c.key.kind, Kind::Node | Kind::Edge) => without_layout(before) == without_layout(after),
            _ => false,
        };
        !self.0.iter().all(layout_only)
    }

    /// ids of the nodes this change touched, by the group they're in (`None` at the top level)
    pub fn nodes(&self) -> Vec<(Option<String>, String)> {
        self.0.iter().filter(|c| c.key.kind == Kind::Node).map(|c| (c.key.scope.clone(), c.key.id.clone())).collect()
    }

    /// puts every record the way it was after the change (`forward`, redo) or before it (undo)
    pub fn apply(&self, project: &mut HashMap<String, Value>, forward: bool) {
        let mut map: Map<String, Value> = std::mem::take(project).into_iter().collect();

        // removed: nodes and edges first, then the groups they were in
        let mut removed: Vec<&Change> = self.0.iter().filter(|c| c.target(forward).0.is_none()).collect();
        removed.sort_by(|a, b| b.key.cmp(&a.key));
        for change in removed {
            remove(&mut map, &change.key);
        }
        // put back or changed: groups before what's inside them, nodes and edges in list order so each lands where it was
        let mut put_back: Vec<(&Change, &Value, usize)> = self.0.iter().filter_map(|c| c.target(forward).0.as_ref().map(|v| (c, v, c.target(forward).1))).collect();
        put_back.sort_by(|a, b| (a.0.key.kind, &a.0.key.scope, a.2).cmp(&(b.0.key.kind, &b.0.key.scope, b.2)));
        for (change, value, at) in put_back {
            put(&mut map, &change.key, value, at);
        }

        *project = map.into_iter().collect();
    }
}

// MARK: - History

/// who made a change
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Ui,
    Mcp,
}

/// what a change is, given when it's recorded
#[derive(Clone, Debug)]
pub struct Step {
    /// the edit operation that made it, e.g. `add_node`
    pub op: String,
    pub source: Source,
    /// what it did in words (MCP edit messages), may be empty
    pub detail: String,
    /// changes with the same open transaction become one entry until it's ended or cancelled (a grab, a duplicate)
    pub txn: Option<String>,
    /// a change with the same merge key as the newest entry joins it (typing into a field)
    pub merge: Option<String>,
}

impl Step {
    pub fn new(op: &str, source: Source) -> Self {
        Self {
            op: op.to_string(),
            source,
            detail: String::new(),
            txn: None,
            merge: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn txn(mut self, txn: impl Into<String>) -> Self {
        self.txn = Some(txn.into());
        self
    }

    pub fn merge(mut self, merge: impl Into<String>) -> Self {
        self.merge = Some(merge.into());
        self
    }
}

/// one undo step
#[derive(Clone, Debug)]
struct Entry {
    id: u64,
    step: Step,
    diff: Diff,
    /// still taking changes from its transaction
    open: bool,
}

/// an entry as the history panel and MCP see it
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct EntryInfo {
    pub id: u64,
    pub op: String,
    pub source: Source,
    pub detail: String,
}

/// the whole history, oldest entry first. the first `current` entries are done, the rest can be redone
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct HistoryInfo {
    pub entries: Vec<EntryInfo>,
    pub current: usize,
}

impl Entry {
    fn info(&self) -> EntryInfo {
        EntryInfo {
            id: self.id,
            op: self.step.op.clone(),
            source: self.step.source,
            detail: self.step.detail.clone(),
        }
    }
}

/// done and undone entries, newest last in both
#[derive(Debug)]
pub struct History {
    done: Vec<Entry>,
    undone: Vec<Entry>,
    next_id: u64,
    pub limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new(DEFAULT_LIMIT)
    }
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            done: Vec::new(),
            undone: Vec::new(),
            next_id: 1,
            limit,
        }
    }

    /// records a change, returns true if the history changed. an empty change records nothing.
    /// joins the newest entry when it's the same open transaction or has the same merge key, otherwise it's a new
    /// entry, which ends any open transaction and drops everything that could be redone
    pub fn record(&mut self, diff: Diff, step: Step) -> bool {
        if diff.is_empty() {
            return false;
        }
        let joins = self.done.last().is_some_and(|top| {
            if top.open {
                step.txn.is_some() && top.step.txn == step.txn
            } else {
                step.merge.is_some() && top.step.merge == step.merge && self.undone.is_empty()
            }
        });
        if joins {
            let top = self.done.last_mut().unwrap();
            top.diff.then(diff);
            // the changes cancelled each other out, as if nothing happened
            if top.diff.is_empty() && !top.open {
                self.done.pop();
            }
            return true;
        }

        // a new entry
        self.end_open();
        self.undone.clear();
        let open = step.txn.is_some();
        self.done.push(Entry {
            id: self.next_id,
            step,
            diff,
            open,
        });
        self.next_id += 1;
        if self.done.len() > self.limit {
            self.done.drain(..self.done.len() - self.limit);
        }
        true
    }

    /// ends the transaction, its entry stops taking changes
    pub fn end(&mut self, txn: &str) {
        if let Some(top) = self.done.last_mut().filter(|top| top.open && top.step.txn.as_deref() == Some(txn)) {
            top.open = false;
        }
        // nothing changed during the transaction
        if self.done.last().is_some_and(|top| top.diff.is_empty()) {
            self.done.pop();
        }
    }

    /// cancels the transaction: its changes are undone and forgotten
    pub fn cancel(&mut self, txn: &str, project: &mut HashMap<String, Value>) -> bool {
        if !self.done.last().is_some_and(|top| top.open && top.step.txn.as_deref() == Some(txn)) {
            return false;
        }
        let entry = self.done.pop().unwrap();
        entry.diff.apply(project, false);
        true
    }

    /// undoes the newest entry, returns it and whether it changes what the graph computes
    pub fn undo(&mut self, project: &mut HashMap<String, Value>) -> Option<(EntryInfo, bool)> {
        self.end_open();
        let entry = self.done.pop()?;
        entry.diff.apply(project, false);
        let stepped = (entry.info(), entry.diff.affects_output());
        self.undone.push(entry);
        Some(stepped)
    }

    /// redoes the newest undone entry, returns it and whether it changes what the graph computes
    pub fn redo(&mut self, project: &mut HashMap<String, Value>) -> Option<(EntryInfo, bool)> {
        self.end_open();
        let entry = self.undone.pop()?;
        entry.diff.apply(project, true);
        let stepped = (entry.info(), entry.diff.affects_output());
        self.done.push(entry);
        Some(stepped)
    }

    /// undoes or redoes until the first `current` entries are done (the history panel's rows), returns whether any of
    /// those steps changes what the graph computes
    pub fn goto(&mut self, current: usize, project: &mut HashMap<String, Value>) -> bool {
        let mut affects_output = false;
        while self.done.len() > current {
            let Some((_, affects)) = self.undo(project) else {
                break;
            };
            affects_output |= affects;
        }
        while self.done.len() < current {
            let Some((_, affects)) = self.redo(project) else {
                break;
            };
            affects_output |= affects;
        }
        affects_output
    }

    /// forgets everything (a project was loaded or a new one started)
    pub fn clear(&mut self) {
        self.done.clear();
        self.undone.clear();
    }

    pub fn info(&self) -> HistoryInfo {
        HistoryInfo {
            entries: self.done.iter().chain(self.undone.iter().rev()).map(Entry::info).collect(),
            current: self.done.len(),
        }
    }

    /// an open transaction is over once anything else happens
    fn end_open(&mut self) {
        if let Some(top) = self.done.last_mut() {
            top.open = false;
        }
        if self.done.last().is_some_and(|top| top.diff.is_empty()) {
            self.done.pop();
        }
    }
}
