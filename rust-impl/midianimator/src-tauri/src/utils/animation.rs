use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub fn sec_to_frames(seconds: f64, fps: f64) -> f64 {
    seconds * fps
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlendKeyframe {
    pub frame: f64,
    pub value: f64,
    pub data_path: String,
    pub array_index: u32,
}

impl BlendKeyframe {
    pub fn new(frame: f64, value: f64, data_path: &str, array_index: u32) -> Self {
        Self {
            frame,
            value,
            data_path: data_path.to_string(),
            array_index,
        }
    }

    pub fn bare(frame: f64, value: f64) -> Self {
        Self {
            frame,
            value,
            data_path: String::new(),
            array_index: 0,
        }
    }
}

// TODO eventually, we will include a file for all node types with structs
#[derive(Debug, Deserialize)]
pub struct AnimationGenerator {
    pub name: String,
    pub note_on_keyframes: Vec<serde_json::Value>,
    pub note_on_anchor_point: f64,
    pub note_off_keyframes: Vec<serde_json::Value>,
    pub note_off_anchor_point: f64,
    pub velocity_intensity: f64,
    pub animation_overlap: String,
    pub animation_property: String,
    // mappers intentionally ignored for now
}

#[derive(Debug, Deserialize)]
pub struct ObjectMapEntry {
    pub note_number: Vec<u8>,
    pub animations: Vec<String>, // name keys into object_map.animations
}

#[derive(Debug, Deserialize)]
pub struct ObjectMap {
    pub animations: HashMap<String, AnimationGenerator>,
    pub objects: HashMap<String, ObjectMapEntry>,
}

pub fn get_value(k1: &BlendKeyframe, k2: &BlendKeyframe, frame: f64) -> f64 {
    let (x1, y1) = (k1.frame, k1.value);
    let (x2, y2) = (k2.frame, k2.value);
    let m = if (x2 - x1).abs() < f64::EPSILON { 0.0 } else { (y2 - y1) / (x2 - x1) };
    m * frame + (y1 - m * x1)
}

pub fn interval<'a>(key_list: &'a [BlendKeyframe], frame: f64) -> (Option<&'a BlendKeyframe>, Option<&'a BlendKeyframe>) {
    if key_list.is_empty() { return (None, None); }
    if key_list[0].frame > frame { return (Some(&key_list[0]), Some(&key_list[0])); }
    let last = &key_list[key_list.len() - 1];
    if last.frame < frame { return (Some(last), Some(last)); }
    for i in 0..key_list.len() - 1 {
        if key_list[i].frame <= frame && frame <= key_list[i + 1].frame {
            return (Some(&key_list[i]), Some(&key_list[i + 1]));
        }
    }
    (None, None)
}

pub fn find_overlap(key_list1: &[BlendKeyframe], key_list2: &[BlendKeyframe]) -> Vec<BlendKeyframe> {
    if key_list1.is_empty() || key_list2.is_empty() { return vec![]; }
    assert!(
        key_list1[0].frame <= key_list2[0].frame,
        "key_list1 starts after key_list2 — notes went backwards in time"
    );
    let first_next = key_list2[0].frame;
    let mut result = vec![];
    let mut found = false;
    for key in key_list1.iter().rev() {
        if key.frame > first_next {
            found = true;
            result.push(key.clone());
        } else {
            if found { result.push(key.clone()); }
            break;
        }
    }
    result.reverse();
    result
}

pub fn add_keyframes(inserted_keys: &mut Vec<BlendKeyframe>, next_keys: &mut Vec<BlendKeyframe>) {
    let overlapping = find_overlap(inserted_keys, next_keys);
    if overlapping.is_empty() {
        inserted_keys.append(next_keys);
        inserted_keys.sort_by(|a, b| a.frame.partial_cmp(&b.frame).unwrap());
        return;
    }

    // Add interpolated overlap values into next_keys
    for key in next_keys.iter_mut() {
        let (i1, i2) = interval(&overlapping, key.frame);
        if let (Some(i1), Some(i2)) = (i1, i2) {
            key.value += get_value(i1, i2, key.frame);
        }
    }

    // Add interpolated next_keys values into the overlapping region of inserted_keys
    let overlapping_frames: Vec<f64> = overlapping.iter().map(|k| k.frame).collect();
    for key in inserted_keys.iter_mut() {
        if overlapping_frames.contains(&key.frame) {
            let (i1, i2) = interval(next_keys, key.frame);
            if let (Some(i1), Some(i2)) = (i1, i2) {
                key.value += get_value(i1, i2, key.frame);
            }
        }
    }

    inserted_keys.append(next_keys);
    inserted_keys.sort_by(|a, b| a.frame.partial_cmp(&b.frame).unwrap());
    inserted_keys.dedup_by(|a, b| (a.frame - b.frame).abs() < f64::EPSILON);
}

// helper to parse properties like "rotation[0]" into ("rotation", 0)
pub fn parse_animation_property(prop: &str) -> (String, u32) {
    if let Some(bracket) = prop.find('[') {
        let data_path = prop[..bracket].to_string();
        let index_str = prop[bracket + 1..].trim_end_matches(']');
        let array_index = index_str.parse::<u32>().unwrap_or(0);
        (data_path, array_index)
    } else {
        (prop.to_string(), 0)
    }
}

pub fn co_from_json(kf: &serde_json::Value) -> Option<(f64, f64)> {
    let co = kf.get("co")?.as_array()?;
    let frame = co.get(0)?.as_f64()?;
    let value = co.get(1)?.as_f64()?;
    Some((frame, value))
}
