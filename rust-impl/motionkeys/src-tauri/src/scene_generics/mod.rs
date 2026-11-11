use serde::{Deserialize, Serialize};

use crate::utils::curve::Key;

// MARK: - Scene
/// the scene the user is working in, as the bridge sends it
#[derive(Serialize, Deserialize, Clone, Debug, schemars::JsonSchema)]
pub struct Scene {
    /// the bridge's id for the scene. empty in scene data saved before scenes had ids
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// the bridge's groups of objects. in Blender: the top-level collections (nested ones flattened in), then the
    /// objects outside every collection
    pub object_groups: Vec<ObjectGroup>,
    /// frames per second, none in scene data saved before it was sent
    #[serde(default)]
    pub fps: Option<f64>,
    /// the scene's axis convention, for the 3d preview. none means z up, right-handed and radians
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axes: Option<Axes>,
    /// the app id of the bridge that sent it (`blender`), set by MotionKeys when it arrives and saved with the tab. none in
    /// scenes saved before it was kept
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// the bridge's name for its app (`Blender`), set and saved with `app`, for messages while nothing is connected
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
}

impl Scene {
    /// what tells scenes apart: the id, or the name in scene data saved before scenes had ids
    pub fn key(&self) -> &str {
        if self.id.is_empty() {
            &self.name
        } else {
            &self.id
        }
    }

    /// every object in every group, an object in two groups comes twice
    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.object_groups.iter().flat_map(|group| group.objects.iter())
    }

    pub fn objects_mut(&mut self) -> impl Iterator<Item = &mut Object> {
        self.object_groups.iter_mut().flat_map(|group| group.objects.iter_mut())
    }
}

/// a scene's axis convention (docs/bridge_protocol.md §8.2), every field optional
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, schemars::JsonSchema)]
pub struct Axes {
    /// `x`, `y` or `z`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub up: Option<String>,
    /// `right` or `left`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handedness: Option<String>,
    /// `radians` or `degrees`, for transforms and the channels tagged `rotation.*`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle_unit: Option<String>,
}

// MARK: - ObjectGroup
#[derive(Serialize, Deserialize, Clone, Debug, schemars::JsonSchema)]
pub struct ObjectGroup {
    /// the bridge's id for the group, stable while it exists. Blender's is `collection:` and the collection's name, or
    /// `scene` for the objects outside every collection. none in scene data saved before groups had ids
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub objects: Vec<Object>,
}

impl ObjectGroup {
    /// what a node saves to pick the group: its id, or its name when it was saved without one
    pub fn key(&self) -> &str {
        if self.id.is_empty() {
            &self.name
        } else {
            &self.id
        }
    }
}

/// the group a node's saved pick means: the one with that id, else the first with that name (picks saved before groups
/// had ids, and names typed in)
pub fn find_group<'a>(groups: &'a [ObjectGroup], pick: &str) -> Option<&'a ObjectGroup> {
    groups.iter().find(|group| group.key() == pick).or_else(|| groups.iter().find(|group| group.name == pick))
}

// MARK: - Object
#[derive(Serialize, Deserialize, Clone, Debug, schemars::JsonSchema)]
pub struct Object {
    /// the bridge's id for the object, stable while it exists. Blender's is the object's name
    pub id: String,
    pub name: String,
    /// what MotionKeys can write to on the object, the object's own channels first. empty in scene data saved before
    /// bridges sent them
    #[serde(default)]
    pub channels: Vec<Channel>,
    /// a snapshot of the object's transform, for display only
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<Transform>,
    /// the euler order of `transform.rotation`, `XYZ` (x first) when it isn't sent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation_order: Option<String>,
    /// the curves of an object MotionKeys reads animation from, filled in from the bridge's curves messages. curves saved
    /// in a form from before are left out, the bridge sends them again
    #[serde(default, deserialize_with = "readable_curves")]
    pub anim_curves: Vec<AnimCurve>,
}

impl Object {
    pub fn channel(&self, id: &str) -> Option<&Channel> {
        self.channels.iter().find(|channel| channel.id == id)
    }

    /// the group of the object's own channels: its first channel's. labels leave it out
    pub fn own_group(&self) -> Option<&str> {
        self.channels.first().map(|channel| channel.group.as_str())
    }

    /// a channel's label where it stands alone (an output's name): its group in front, `Arm › Location X`, except for
    /// the object's own channels. a channel the bridge didn't list is its id
    pub fn channel_label(&self, id: &str) -> String {
        match self.channel(id) {
            Some(channel) if Some(channel.group.as_str()) == self.own_group() || channel.group.is_empty() => channel.title(),
            Some(channel) => format!("{} › {}", channel.group, channel.title()),
            None => id.to_string(),
        }
    }
}

/// the object a node's saved pick means: the one with that id, else the first with that name (picks typed in)
pub fn find_object<'a>(objects: &'a [Object], pick: &str) -> Option<&'a Object> {
    objects.iter().find(|object| object.id == pick).or_else(|| objects.iter().find(|object| object.name == pick))
}

/// an object's transform: local, in the 3d app's units, rotation in the scene's angle unit and the object's
/// `rotation_order`
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Transform {
    pub position: [f64; 3],
    pub rotation: [f64; 3],
    pub scale: [f64; 3],
}

// MARK: - Channel
/// one animatable value on an object (docs/bridge_protocol.md §8.5). its id is the bridge's, never parsed
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Channel {
    pub id: String,
    /// the name without the axis, `Location`
    pub name: String,
    /// what it's listed under, `Object`, a bone's name or `Shape Keys`
    #[serde(default)]
    pub group: String,
    /// `X`, `Y`, `Z` or `W` for one component of a vector
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis: Option<String>,
    /// what it moves, for the 3d preview: `position.x`, `rotation.z`, `quaternion.w`, `scale.y`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic: Option<String>,
    /// has keys now
    #[serde(default)]
    pub animated: bool,
}

impl Channel {
    /// the name then the axis, `Location Z`
    pub fn title(&self) -> String {
        match &self.axis {
            Some(axis) => format!("{} {}", self.name, axis),
            None => self.name.clone(),
        }
    }

    /// a custom property's channel id, the same in every bridge: `["wear"]`, with `\` and `"` in the name escaped. the
    /// one id MotionKeys builds itself, each bridge translates it to its app's own path (docs/bridge_protocol.md §8.5)
    pub fn custom_property_id(name: &str) -> String {
        format!("[\"{}\"]", name.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// the curves that read as curves, the others are left out
fn readable_curves<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<AnimCurve>, D::Error> {
    let curves = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(curves.into_iter().filter_map(|curve| serde_json::from_value(curve).ok()).collect())
}

// MARK: - AnimCurve
/// one channel's keys, as bridges send and write them (docs/bridge_protocol.md §8.6)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct AnimCurve {
    /// the bridge's id for the channel, `location[2]` in Blender
    pub channel: String,
    /// `[time, value, interpolation]` or `[time, value, interpolation, left_time, left_value, right_time, right_value]`
    #[serde(with = "crate::utils::curve::wire_keys")]
    #[schemars(with = "Vec<Vec<serde_json::Value>>")]
    pub keys: Vec<Key>,
}

// MARK: - Keyframe
#[derive(Serialize, Deserialize, Clone, Debug, schemars::JsonSchema)]
pub struct Keyframe {
    pub time: f32,
    pub value: f32,
}
