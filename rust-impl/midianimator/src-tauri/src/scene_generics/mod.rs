// use nalgebra::Vector3;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

// MARK: - Scene
#[derive(Serialize, Deserialize, Debug)]
pub struct Scene {
    pub name: String,
    pub object_groups: Vec<ObjectGroup>,
}

// MARK: - ObjectGroup
#[derive(Serialize, Deserialize, Debug)]
pub struct ObjectGroup {
    pub name: String,
    pub objects: Vec<Object>,
}

// MARK: - Object
#[derive(Serialize, Deserialize, Debug)]
pub struct Object {
    pub name: String,
    pub position: Vector3,
    pub rotation: Vector3,
    pub scale: Vector3,
    pub blend_shapes: BlendShapes,
    pub anim_curves: Vec<AnimCurve>,
    // pub mesh: Mesh
}

// MARK: - AnimCurve
#[derive(Serialize, Deserialize, Debug)]
pub struct AnimCurve {
    pub array_index: u32,
    pub auto_smoothing: String,
    pub data_path: String,
    pub extrapolation: String,
    pub keyframe_points: Vec<KeyframePoint>,
    pub range: Vec<f32>,
}

// MARK: - BlendShape
#[derive(Serialize, Deserialize, Debug)]
pub struct BlendShapes {
    pub keys: Vec<String>,
    pub reference: Option<String>,
}

// MARK: - Keyframe
#[derive(Serialize, Deserialize, Debug)]
pub struct Keyframe {
    pub time: f32,
    pub value: f32,
}

// MARK: - KeyframePoint
#[derive(Serialize, Deserialize, Debug)]
pub struct KeyframePoint {
    pub amplitude: f32,
    pub back: f32,
    pub easing: String,
    pub handle_left: Vec<f32>,
    pub handle_left_type: String,
    pub handle_right: Vec<f32>,
    pub handle_right_type: String,
    pub interpolation: String,
    pub co: Vec<f32>,
    pub period: f32,
}
