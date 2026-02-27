// src/graph/types.rs
use std::collections::HashMap;

// TODO WIP

// MARK: - Primitives
struct MKString {
    value: String
}

struct MKF64 {
    value: f64
}

struct MKAny {
    value: serde_json::Value
}

// MARK: - Collections
struct MKArray {
    value: Vec<serde_json::Value>
}

struct MKHashMap {
    value: HashMap<serde_json::Value, serde_json::Value>
}

// MARK: - Custom types
struct MKKeyframe {
    frame: f64,
    value: f64,
}

struct MKObjectGroup {
    name: String,
    objects: Vec<String>,
}

struct MKAnimationGenerator {
    name: String,
    note_on_keyframes: Vec<serde_json::Value>,
    note_on_anchor_point: f64,
    note_off_keyframes: Vec<serde_json::Value>,
    note_off_anchor_point: f64,
    velocity_intensity: f64,
    animation_overlap: String,
    animation_property: String,
}

struct MKObjectMap {
    animations: HashMap<String, MKAnimationGenerator>,
    objects: HashMap<String, MKObjectGroup>,
}