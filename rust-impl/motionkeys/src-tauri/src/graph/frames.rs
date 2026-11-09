// frames: labeled boxes that hold nodes, like Blender's frame node. a frame is a node of type `frame` with no sockets,
// a node in one names it in its `frame` field (a frame in a frame the same way). positions stay absolute: a frame being
// somewhere doesn't move what's in it, moving it moves each node in it (the editor sends every position).
// a frame fits around what's in it like a for each zone. resizing one turns off Shrink (Blender's): it keeps at least the
// size it was given, and grows past it to take in its nodes. Lock Size keeps exactly its size, what's in it can stick
// out. a locked frame stays where it is, behind the graph: only a click on its label selects it.
// frames are only layout, running skips them and changing them never re-runs the graph

use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

use super::model::{Graph, Position, RfNode};
use super::run::FRAME;

/// room around what's in a shrinking frame. keep in sync with src/utils/frames.ts
pub const PAD: f64 = 20.0;
/// the label's size when none is set, and the smallest and largest it can be (Blender's). keep in sync with src/utils/frames.ts
pub const LABEL_SIZE: f64 = 20.0;
const LABEL_SIZES: (f64, f64) = (8.0, 64.0);
/// room above and below the label in its strip at the top of the frame. keep in sync with src/utils/frames.ts
const LABEL_PAD: f64 = 16.0;
/// where the label goes along the top
const LABEL_ALIGNS: &[&str] = &["left", "center", "right"];
/// a new frame's color, a gray that shows on every theme. keep in sync with src/utils/frames.ts
pub const DEFAULT_COLOR: &str = "#999999";
/// a node's size when the editor hasn't said what it measured
const NODE_WIDTH: f64 = 200.0;
const NODE_HEIGHT: f64 = 100.0;

/// a node's size as the editor measured it
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// the box a node takes up on the canvas
#[derive(Debug, Clone, Copy)]
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

/// whether a frame fits tight around what's in it, off once it was given a size (resized)
pub fn shrinks(frame: &RfNode) -> bool {
    frame.data.get("shrink").and_then(Value::as_bool) != Some(false)
}

/// whether a frame keeps exactly its size
pub fn locks_size(frame: &RfNode) -> bool {
    frame.data.get("lock_size").and_then(Value::as_bool) == Some(true)
}

/// how tall the strip with the label is, by the label's size
pub fn label_height(frame: &RfNode) -> f64 {
    frame.data.get("label_size").and_then(Value::as_f64).unwrap_or(LABEL_SIZE) + LABEL_PAD
}

/// the frames a node is in, innermost first. a loop (a broken project) is cut where it comes back around
pub fn ancestors(graph: &Graph, id: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut at = graph.node(id).and_then(RfNode::frame);
    while let Some(frame) = at {
        if frame == id || out.iter().any(|f| f == frame) {
            break;
        }
        out.push(frame.to_string());
        at = graph.node(frame).and_then(RfNode::frame);
    }
    out
}

/// the nodes among `ids` and everything inside the frames among them, at any depth, in graph order
pub fn with_contents(graph: &Graph, ids: &[String]) -> Vec<String> {
    let picked: HashSet<&str> = ids.iter().map(String::as_str).collect();
    graph.nodes.iter().filter(|n| picked.contains(n.id.as_str()) || ancestors(graph, &n.id).iter().any(|f| picked.contains(f.as_str()))).map(|n| n.id.clone()).collect()
}

/// the nodes among `ids` that aren't inside a frame among them, they're the ones that move from frame to frame
fn roots(graph: &Graph, ids: &HashSet<&str>) -> Vec<String> {
    graph.nodes.iter().filter(|n| ids.contains(n.id.as_str()) && !ancestors(graph, &n.id).iter().any(|f| ids.contains(f.as_str()))).map(|n| n.id.clone()).collect()
}

// MARK: - Fit

/// where a node is and how big: a frame's own size, what the editor measured, a resized node's size, or a guess
fn rect(node: &RfNode, sizes: &HashMap<String, Size>) -> Rect {
    let own = |key: &str| node.extra.get(key).and_then(Value::as_f64);
    let measured = sizes.get(&node.id).filter(|_| !node.is_frame());
    let width = measured.map(|s| s.width).or_else(|| own("width")).unwrap_or(NODE_WIDTH);
    let height = measured.map(|s| s.height).or_else(|| own("height")).unwrap_or(NODE_HEIGHT);
    Rect {
        x0: node.position.x,
        y0: node.position.y,
        x1: node.position.x + width,
        y1: node.position.y + height,
    }
}

/// what each frame's fit depends on: where everything in it is and which frame it's in, and whether it shrinks.
/// taken before an op, `settle` refits the frames whose layout it changed
pub fn layouts(graph: &Graph) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    for frame in graph.nodes.iter().filter(|n| n.is_frame()) {
        let mut inside: Vec<String> = with_contents(graph, &[frame.id.clone()]).into_iter().filter(|id| *id != frame.id).filter_map(|id| graph.node(&id)).map(|n| format!("{}@{},{}<{}", n.id, n.position.x, n.position.y, n.frame().unwrap_or(""))).collect();
        inside.sort();
        out.insert(frame.id.clone(), format!("{}|{}|{}|{}", shrinks(frame), locks_size(frame), label_height(frame), inside.join(";")));
    }
    out
}

/// after an op: what's in a frame that's gone is taken out, and the frames whose layout changed since `before`
/// (`layouts`) fit around what's in them again, innermost first
pub fn settle(graph: &mut Graph, sizes: &HashMap<String, Size>, before: &HashMap<String, String>) {
    sync(graph);
    let after = layouts(graph);
    let mut dirty: Vec<(usize, String)> = after.iter().filter(|(id, layout)| before.get(*id) != Some(*layout)).map(|(id, _)| (ancestors(graph, id).len(), id.clone())).collect();
    dirty.sort_by(|a, b| b.cmp(a));
    for (_, id) in dirty {
        fit(graph, &id, sizes);
    }
}

/// the box around two rects
fn union(a: Rect, b: Rect) -> Rect {
    Rect {
        x0: a.x0.min(b.x0),
        y0: a.y0.min(b.y0),
        x1: a.x1.max(b.x1),
        y1: a.y1.max(b.y1),
    }
}

/// fits a frame around what's directly in it with room for its label, one given a size keeps at least that. a frame
/// with nothing in it or a locked size stays as it is. keep in sync with frameRects in frames.ts
fn fit(graph: &mut Graph, id: &str, sizes: &HashMap<String, Size>) {
    let Some(inside) = graph.nodes.iter().filter(|n| n.frame() == Some(id)).map(|n| rect(n, sizes)).reduce(union) else {
        return;
    };
    let Some(frame) = graph.node_mut(id).filter(|f| !locks_size(f)) else {
        return;
    };
    let fitted = Rect {
        x0: inside.x0 - PAD,
        y0: inside.y0 - PAD - label_height(frame),
        x1: inside.x1 + PAD,
        y1: inside.y1 + PAD,
    };
    let bounds = if shrinks(frame) {
        fitted
    } else {
        union(fitted, rect(frame, sizes))
    };
    frame.position = Position {
        x: bounds.x0,
        y: bounds.y0,
    };
    frame.extra.insert("width".to_string(), json!(bounds.x1 - bounds.x0));
    frame.extra.insert("height".to_string(), json!(bounds.y1 - bounds.y0));
}

/// drops a frame that no longer holds: a node in a frame that's gone (or isn't a frame) is taken out, and so is a frame
/// that ends up inside itself
pub fn sync(graph: &mut Graph) {
    let frames: HashSet<String> = graph.nodes.iter().filter(|n| n.is_frame()).map(|n| n.id.clone()).collect();
    let looped: HashSet<String> = frames.iter().filter(|id| ancestors(graph, id).last().is_some_and(|top| graph.node(top).and_then(RfNode::frame) == Some(id.as_str()))).cloned().collect();
    for node in &mut graph.nodes {
        if node.frame().is_some_and(|f| !frames.contains(f) || f == node.id || looped.contains(&node.id)) {
            node.set_frame(None);
        }
    }
}

// MARK: - Edits

/// puts nodes in a new frame (Shift+P) and returns its id. a node inside a frame that's joined too stays in it, the new
/// frame goes in the innermost frame around all the others. `settle` sizes it
pub fn join(graph: &mut Graph, nodes: &[String]) -> Result<String, String> {
    let picked: HashSet<&str> = nodes.iter().map(String::as_str).filter(|id| graph.node(id).is_some()).collect();
    let roots = roots(graph, &picked);
    if roots.is_empty() {
        return Err("nothing to join in a frame".to_string());
    }

    // the innermost frame every one of them is in
    let chains: Vec<Vec<String>> = roots.iter().map(|id| ancestors(graph, id)).collect();
    let parent = chains[0].iter().find(|f| chains.iter().all(|chain| chain.contains(f))).cloned();

    let id = graph.next_node_id(FRAME);
    let mut frame = RfNode {
        id: id.clone(),
        node_type: FRAME.to_string(),
        position: Position::default(),
        data: Map::from_iter([("color".to_string(), json!(DEFAULT_COLOR))]),
        extra: Map::new(),
    };
    frame.set_frame(parent.as_deref());
    graph.nodes.push(frame);
    for root in &roots {
        graph.node_mut(root).unwrap().set_frame(Some(&id));
    }
    Ok(id)
}

/// takes nodes out of their frames (Alt+P) into the frame around it, if any. a node inside a frame that's taken out
/// too stays in it. a frame they all came out of goes, there's nothing left to frame (`drop_emptied`)
pub fn remove_from(graph: &mut Graph, nodes: &[String]) -> Result<(), String> {
    let picked: HashSet<&str> = nodes.iter().map(String::as_str).collect();
    let moves: Vec<(String, String, Option<String>)> = roots(graph, &picked).into_iter().filter_map(|id| graph.node(&id).and_then(RfNode::frame).map(|f| (id.clone(), f.to_string(), graph.node(f).and_then(RfNode::frame).map(str::to_string)))).collect();
    if moves.is_empty() {
        return Err("none of the nodes are in a frame".to_string());
    }
    let left: HashSet<String> = moves.iter().map(|(_, from, _)| from.clone()).collect();
    for (id, _, outer) in moves {
        graph.node_mut(&id).unwrap().set_frame(outer.as_deref());
    }
    drop_emptied(graph, left);
    Ok(())
}

/// puts nodes in the frame each was dropped in, `None` for none. a node that's gone is skipped and a frame that's gone is
/// none (something else removed them while the drop was on its way, like a move skips nodes that are gone)
pub fn place(graph: &mut Graph, frames: &BTreeMap<String, Option<String>>) -> Result<(), String> {
    for (id, frame) in frames {
        if graph.node(id).is_none() {
            continue;
        }
        let frame = frame.as_deref().filter(|frame| graph.node(frame).is_some());
        if let Some(frame) = frame {
            if !graph.node(frame).is_some_and(RfNode::is_frame) {
                return Err(format!("'{}' is not a frame", frame));
            }
            // a frame can't go inside itself
            if frame == id.as_str() || ancestors(graph, frame).contains(id) {
                return Err(format!("'{}' can't go inside '{}'", id, frame));
            }
        }
        graph.node_mut(id).unwrap().set_frame(frame);
    }
    Ok(())
}

/// removes nodes (deleting, cutting). a removed frame's nodes stay, in the frame around it like Blender keeps them, and a
/// frame left with nothing in it goes too unless `keep_emptied`. their connections are the caller's
pub fn remove(graph: &mut Graph, removed: &HashSet<String>, keep_emptied: bool) {
    release(graph, removed);
    let held: HashSet<String> = graph.nodes.iter().filter(|n| removed.contains(&n.id)).filter_map(RfNode::frame).filter(|f| !removed.contains(*f)).map(str::to_string).collect();
    graph.nodes.retain(|n| !removed.contains(&n.id));
    if !keep_emptied {
        drop_emptied(graph, held);
    }
}

/// removes the frames among `frames` with nothing left in them, and then the frames around those if that empties them
fn drop_emptied(graph: &mut Graph, mut frames: HashSet<String>) {
    while let Some(id) = frames.iter().find(|f| !graph.nodes.iter().any(|n| n.frame() == Some(f.as_str()))).cloned() {
        frames.remove(&id);
        if let Some(outer) = graph.node(&id).and_then(RfNode::frame) {
            frames.insert(outer.to_string());
        }
        graph.nodes.retain(|n| n.id != id);
    }
}

/// before frames are removed: what's in them goes to the frame around them
fn release(graph: &mut Graph, removed: &HashSet<String>) {
    let moves: Vec<(String, Option<String>)> = graph.nodes.iter().filter(|n| !removed.contains(&n.id) && n.frame().is_some_and(|f| removed.contains(f))).map(|n| (n.id.clone(), ancestors(graph, &n.id).into_iter().find(|f| !removed.contains(f)))).collect();
    for (id, frame) in moves {
        graph.node_mut(&id).unwrap().set_frame(frame.as_deref());
    }
}

/// what set_frame changes on a frame, what's left out stays as it is
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Style {
    #[serde(default)]
    pub color: Option<String>,
    /// fits tight around what's in it, off keeps at least the size it was given
    #[serde(default)]
    pub shrink: Option<bool>,
    /// keeps exactly its size
    #[serde(default)]
    pub lock_size: Option<bool>,
    /// can't be moved, resized or box selected, only a click on its label selects it
    #[serde(default)]
    pub locked: Option<bool>,
    #[serde(default)]
    pub label_size: Option<f64>,
    /// left, center or right
    #[serde(default)]
    pub label_align: Option<String>,
}

/// changes how a frame looks and fits. a flag is only kept while it's not its default
pub fn set(graph: &mut Graph, id: &str, style: &Style) -> Result<(), String> {
    if style.label_align.as_deref().is_some_and(|align| !LABEL_ALIGNS.contains(&align)) {
        return Err(format!("a label goes {}", LABEL_ALIGNS.join(", ")));
    }
    let frame = graph.node_mut(id).filter(|n| n.is_frame()).ok_or_else(|| format!("'{}' is not a frame", id))?;
    if let Some(color) = &style.color {
        frame.data.insert("color".to_string(), json!(color));
    }
    let mut put = |key: &str, value: Option<Value>, default: Value| {
        match value {
            Some(value) if value == default => frame.data.remove(key),
            Some(value) => frame.data.insert(key.to_string(), value),
            None => None,
        };
    };
    put("shrink", style.shrink.map(|v| json!(v)), json!(true));
    put("lock_size", style.lock_size.map(|v| json!(v)), json!(false));
    put("locked", style.locked.map(|v| json!(v)), json!(false));
    put("label_size", style.label_size.map(|v| json!(v.clamp(LABEL_SIZES.0, LABEL_SIZES.1))), json!(LABEL_SIZE));
    put("label_align", style.label_align.as_ref().map(|a| json!(a)), json!("left"));
    Ok(())
}
