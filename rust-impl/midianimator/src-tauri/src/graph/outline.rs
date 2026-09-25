// text outline of the node graph and short summaries of values, written for an ML model to read (over MCP).
// the UI still shows the full data, nothing in here is used by the frontend

use serde_json::Value;
use std::collections::HashMap;

use super::model::{find_spec, is_param, node_outputs, Graph, HandleSpec, NodeSpec, RfNode};
use crate::midi::MIDINote;
use crate::scene_generics::Scene;

/// everything needed to describe the graph, borrowed from a snapshot of `AppState`
pub struct OutlineCtx<'a> {
    pub graph: &'a Graph,
    pub specs: &'a [NodeSpec],
    /// `executed_results`: `{node_id: {output_id: value}}`
    pub results: &'a HashMap<String, Value>,
    /// `executed_inputs`: `{node_id: {input_id: value}}`
    pub inputs: &'a HashMap<String, Value>,
    pub scene_data: &'a HashMap<String, Scene>,
}

/// how much to show in an outline, `Full` adds descriptions and the values coming in over connections
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Concise,
    Full,
}

// max number of options listed for an input before we cut it off
const MAX_OPTIONS: usize = 20;

// MARK: - Outline

/// outline of the whole graph (or, with `scope`, of the nodes upstream and downstream of one node),
/// one block per node, producers before consumers
pub fn outline(ctx: &OutlineCtx, scope: Option<&str>, detail: Detail) -> Result<String, String> {
    // nothing to outline, tell the caller where to start
    if ctx.graph.nodes.is_empty() {
        return Ok("the graph is empty; call node_types_list, then graph_add_node".to_string());
    }

    // order the nodes so producers come before consumers
    let mut ids = ctx.graph.topo_order();
    // if scoped, only keep the node itself and anything upstream or downstream of it
    if let Some(scope) = scope {
        let scope_id = ctx.graph.resolve(scope)?;
        ids.retain(|id| *id == scope_id || ctx.graph.reaches(id, &scope_id) || ctx.graph.reaches(&scope_id, id));
    }

    // one block per node, separated by a blank line
    let blocks: Vec<String> = ids.iter().map(|id| node_block(ctx, id, detail)).collect();
    Ok(blocks.join("\n\n"))
}

/// outline block for one node
pub fn node_block(ctx: &OutlineCtx, id: &str, detail: Detail) -> String {
    // bail out early if the node is gone or its type is unknown
    let Some(node) = ctx.graph.node(id) else {
        return format!("{}  (removed)", id);
    };
    let Some(spec) = find_spec(ctx.specs, node.resolved_node_type()) else {
        return format!("{}  (unknown node type '{}')", id, node.resolved_node_type());
    };
    // results are only there if the node has executed
    let node_results = ctx.results.get(id);

    // header line: id, name and whether it has executed yet
    let mut header = format!("{}  \"{}\"", id, spec.name);
    if node_results.is_none() {
        header.push_str(if spec.realtime {
            "  (not executed)"
        } else {
            "  (not executed; runs only with graph_execute write_to_blender=true)"
        });
    }
    // add the node description in full detail
    let mut lines = vec![header];
    if detail == Detail::Full && !spec.description.is_empty() {
        lines.push(format!("  # {}", spec.description));
    }

    // one line per input, in concise mode unset inputs get grouped into one line at the end
    let mut unset: Vec<&str> = Vec::new();
    for input in &spec.handles.inputs {
        // parameters (hidden inputs) are marked `par`, regular inputs `in`
        let param = is_param(spec, &input.id);
        let kind = if param {
            "par"
        } else {
            "in "
        };
        let mut line = format!("  {}  {}  ({}: {})", kind, input.name, input.id, input.data_type);

        // an input either comes from a connection, a value set on the node, or isn't set
        let edge = ctx.graph.edge_into(id, &input.id);
        let value = node.input_value(&input.id);
        // connected, show where it comes from (and the value that came in, in full detail)
        if let Some(edge) = edge {
            line.push_str(&format!("  <- {} › {}", edge.from_node(), output_name(ctx, edge.from_node(), edge.from_output())));
            if detail == Detail::Full {
                if let Some(v) = ctx.inputs.get(id).and_then(|i| i.get(&input.id)) {
                    line.push_str(&format!("   [{}]", summarize(&input.data_type, v)));
                }
            }
        } else if let Some(value) = value {
            // set on the node itself
            line.push_str(&format!(" = {}", truncate(&value.to_string(), 200)));
        } else if detail == Detail::Concise && !param {
            // unset, collect it for the `unset:` line instead
            unset.push(&input.id);
            continue;
        } else {
            // unset parameters (or anything unset in full detail) get their own line
            line.push_str("  (not set)");
        }

        // list the valid values for this input, if we know them
        if let Some(options) = input_options(ctx, node, &input.id) {
            if options.is_empty() {
                line.push_str("   options: none yet (connect and execute upstream first)");
            } else {
                // quote names that would be ambiguous in a comma separated list
                let shown: Vec<String> = options
                    .iter()
                    .take(MAX_OPTIONS)
                    .map(|s| {
                        if s.contains(',') {
                            format!("{:?}", s)
                        } else {
                            s.clone()
                        }
                    })
                    .collect();
                let more = if options.len() > MAX_OPTIONS {
                    format!(", … ({} total)", options.len())
                } else {
                    String::new()
                };
                line.push_str(&format!("   options: {}{}", shown.join(", "), more));
            }
        }
        lines.push(line);
        push_description(&mut lines, input, detail);
    }
    // all the unset inputs on one line
    if !unset.is_empty() {
        lines.push(format!("  unset: {}", unset.join(", ")));
    }

    // one line per output, including any dynamic ones
    let outputs = node_outputs(spec, node_results);
    for output in &outputs {
        let mut line = format!("  out  {}  ({}: {})", output.name, output.id, output.data_type);
        if output.hidden {
            line.push_str("  hidden, do not connect");
        }
        // where this output is connected to
        let targets: Vec<String> = ctx.graph.edges_from(id, Some(&output.id)).map(|e| format!("{} › {}", e.to_node(), input_name(ctx, e.to_node(), e.to_input()))).collect();
        if !targets.is_empty() {
            line.push_str(&format!("  -> {}", targets.join(", ")));
        }
        // show a summary of the value if the node has executed
        if let Some(results) = node_results {
            match results.get(&output.id) {
                Some(value) => line.push_str(&format!("   [{}]", summarize(&output.data_type, value))),
                None => line.push_str("   [no value]"),
            }
        }
        lines.push(line);
        push_description(&mut lines, output, detail);
    }

    // connections from dynamic outputs we don't know about yet (the node needs to execute again)
    for edge in ctx.graph.edges_from(id, None).filter(|e| !outputs.iter().any(|o| o.id == e.from_output())) {
        lines.push(format!("  out  {}  (dynamic, not executed yet)  -> {} › {}", edge.from_output(), edge.to_node(), input_name(ctx, edge.to_node(), edge.to_input())));
    }

    lines.join("\n")
}

/// adds a handle's description under its line, only in full detail
fn push_description(lines: &mut Vec<String>, handle: &HandleSpec, detail: Detail) {
    if detail == Detail::Full && !handle.description.is_empty() {
        lines.push(format!("         # {}", handle.description));
    }
}

/// display name of an output, falls back to the id if the node or output can't be found
fn output_name(ctx: &OutlineCtx, node_id: &str, output_id: &str) -> String {
    ctx.graph.node(node_id).and_then(|n| find_spec(ctx.specs, n.resolved_node_type())).and_then(|spec| node_outputs(spec, ctx.results.get(node_id)).into_iter().find(|h| h.id == output_id)).map(|h| h.name).unwrap_or_else(|| output_id.to_string())
}

/// display name of an input, falls back to the id if the node or input can't be found
fn input_name(ctx: &OutlineCtx, node_id: &str, input_id: &str) -> String {
    ctx.graph.node(node_id).and_then(|n| find_spec(ctx.specs, n.resolved_node_type())).and_then(|spec| spec.input(input_id)).map(|h| h.name.clone()).unwrap_or_else(|| input_id.to_string())
}

/// inputs that are neither connected nor set on the node
pub fn unset_inputs(ctx: &OutlineCtx, id: &str) -> Vec<String> {
    // bail out if the node is gone or its type is unknown
    let Some(node) = ctx.graph.node(id) else {
        return vec![];
    };
    let Some(spec) = find_spec(ctx.specs, node.resolved_node_type()) else {
        return vec![];
    };
    // keep inputs with no edge and no value
    spec.handles.inputs.iter().filter(|h| ctx.graph.edge_into(id, &h.id).is_none() && node.input_value(&h.id).is_none()).map(|h| h.id.clone()).collect()
}

// MARK: - Options

/// valid values for parameter inputs, where they can be derived from executed data or the scene
pub fn input_options(ctx: &OutlineCtx, node: &RfNode, input_id: &str) -> Option<Vec<String>> {
    // the executed inputs are where upstream data (tracks, object groups) shows up
    let executed = ctx.inputs.get(&node.id);
    match (node.resolved_node_type(), input_id) {
        // track names come from the tracks fed into the node
        ("get_midi_track_data", "track_name") => Some(names(executed.and_then(|i| i.get("tracks")))),
        // object group names come from the executed inputs or the scene
        ("keyframes_from_object", "object_group_name") | ("assign_notes_to_objects", "object_group_name") => Some(object_groups(ctx, node).iter().filter_map(|g| g.get("name").and_then(|n| n.as_str()).map(String::from)).collect()),
        // object names come from the object group picked on the node
        ("keyframes_from_object", "object_name") => {
            let group_name = node.input_value("object_group_name").and_then(|v| v.as_str()).unwrap_or("");
            let groups = object_groups(ctx, node);
            let group = groups.iter().find(|g| g.get("name").and_then(|n| n.as_str()) == Some(group_name));
            Some(names(group.and_then(|g| g.get("objects"))))
        }
        // no known options for this input
        _ => None,
    }
}

/// `name` fields of an array of objects
fn names(array: Option<&Value>) -> Vec<String> {
    array.and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|item| item.get("name").and_then(|n| n.as_str()).map(String::from)).collect()).unwrap_or_default()
}

/// object groups the node sees: from its executed inputs, else every group in the scene data
fn object_groups(ctx: &OutlineCtx, node: &RfNode) -> Vec<Value> {
    // use the groups fed into the node if it has executed
    if let Some(groups) = ctx.inputs.get(&node.id).and_then(|i| i.get("object_groups")).and_then(|g| g.as_array()) {
        return groups.clone();
    }
    // otherwise, every object group in every scene
    ctx.scene_data.values().flat_map(|scene| scene.object_groups.iter()).filter_map(|g| serde_json::to_value(g).ok()).collect()
}

// MARK: - Summaries

/// one-line summary of a value of the given node type, for ML consumption
pub fn summarize(data_type: &str, value: &Value) -> String {
    // use a specific summary for types we know about, otherwise fall back to a generic one
    let specific = match data_type {
        "Array<MIDINote>" => summarize_notes(value),
        "Array<MIDITrack>" => summarize_tracks(value),
        "Array<ObjectGroup>" => summarize_object_groups(value),
        "Array<Keyframe>" => summarize_curve(value),
        _ => None,
    };
    specific.unwrap_or_else(|| summarize_json(value))
}

/// `412 notes · C2–G6 · 0.0–83.2 s`
fn summarize_notes(value: &Value) -> Option<String> {
    // parse the notes, if it fails fall back to the generic summary
    let notes: Vec<MIDINote> = serde_json::from_value(value.clone()).ok()?;
    if notes.is_empty() {
        return Some("0 notes".to_string());
    }
    // lowest/highest note and first/last time
    let low = notes.iter().map(|n| n.note_number).min()?;
    let high = notes.iter().map(|n| n.note_number).max()?;
    let start = notes.iter().map(|n| n.time_on).fold(f64::INFINITY, f64::min);
    let end = notes.iter().map(|n| n.time_off).fold(f64::NEG_INFINITY, f64::max);
    Some(format!("{} notes · {}–{} · {:.1}–{:.1} s", notes.len(), note_name(low), note_name(high), start, end))
}

/// `2 tracks: Studio Grand (412 notes), Drums (120 notes)`
fn summarize_tracks(value: &Value) -> Option<String> {
    // list the first 10 tracks, then "…" if there are more
    let tracks = value.as_array()?;
    let mut parts: Vec<String> = tracks
        .iter()
        .take(10)
        .map(|t| {
            let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            let count = t.get("notes").and_then(|n| n.as_array()).map_or(0, |n| n.len());
            format!("{} ({} notes)", name, count)
        })
        .collect();
    if tracks.len() > 10 {
        parts.push("…".to_string());
    }
    Some(format!(
        "{} track{}: {}",
        tracks.len(),
        if tracks.len() == 1 {
            ""
        } else {
            "s"
        },
        parts.join(", ")
    ))
}

/// `1 group: Cubes (6 objects)`
fn summarize_object_groups(value: &Value) -> Option<String> {
    // list each group with its object count
    let groups = value.as_array()?;
    let parts: Vec<String> = groups
        .iter()
        .map(|g| {
            let name = g.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            let count = g.get("objects").and_then(|o| o.as_array()).map_or(0, |o| o.len());
            format!("{} ({} objects)", name, count)
        })
        .collect();
    Some(format!(
        "{} group{}: {}",
        groups.len(),
        if groups.len() == 1 {
            ""
        } else {
            "s"
        },
        parts.join(", ")
    ))
}

/// keyframe outputs are Blender F-curves: `location[2] · 3 keyframes · frames 0–10`
fn summarize_curve(value: &Value) -> Option<String> {
    // get the curve info and the frames of its keyframes
    let points = value.get("keyframe_points")?.as_array()?;
    let data_path = value.get("data_path").and_then(|v| v.as_str()).unwrap_or("?");
    let index = value.get("array_index").and_then(|v| v.as_u64()).unwrap_or(0);
    let frames: Vec<f64> = points.iter().filter_map(|p| p.get("co")?.get(0)?.as_f64()).collect();
    // frame range, only if there are keyframes
    let range = if frames.is_empty() {
        String::new()
    } else {
        let lo = frames.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = frames.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        format!(" · frames {}–{}", lo, hi)
    };
    Some(format!("{}[{}] · {} keyframes{}", data_path, index, points.len(), range))
}

/// MIDI note number to name, 60 = C4
pub fn note_name(note: u8) -> String {
    // octave numbering where 60 is C4
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", NAMES[(note % 12) as usize], (note as i32) / 12 - 1)
}

/// generic fallback: arrays show length and the first items, objects show keys, strings are truncated
pub fn summarize_json(value: &Value) -> String {
    match value {
        // strings: quoted, single line and truncated
        Value::String(s) => format!("\"{}\"", truncate(&s.replace('\n', "\\n"), 60)),
        // arrays: count and the first 3 items
        Value::Array(items) => {
            if items.is_empty() {
                return "0 items".to_string();
            }
            let first: Vec<String> = items.iter().take(3).map(|item| truncate(&item.to_string(), 40)).collect();
            let more = if items.len() > 3 {
                ", …"
            } else {
                ""
            };
            format!(
                "{} item{}: [{}{}]",
                items.len(),
                if items.len() == 1 {
                    ""
                } else {
                    "s"
                },
                first.join(", "),
                more
            )
        }
        // objects: the first 8 keys and the key count
        Value::Object(map) => {
            let keys: Vec<&str> = map.keys().take(8).map(|k| k.as_str()).collect();
            let more = if map.len() > 8 {
                ", …"
            } else {
                ""
            };
            format!(
                "{{{}{}}} ({} key{})",
                keys.join(", "),
                more,
                map.len(),
                if map.len() == 1 {
                    ""
                } else {
                    "s"
                }
            )
        }
        // numbers, bools and null are printed as is
        other => other.to_string(),
    }
}

/// cuts a string to `max_chars` characters and adds "…" if it was longer
fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars).collect::<String>())
    }
}
