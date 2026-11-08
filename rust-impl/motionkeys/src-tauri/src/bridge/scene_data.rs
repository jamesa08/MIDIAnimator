use crate::graph::execute::run_instance;
use crate::ipc;
use crate::scene_generics::{ObjectGroup, Scene};
use crate::state::{lock, update_state};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;

/// the scene the user is working in, from the bridge
pub async fn get_scene() -> Result<Scene, String> {
    let body = ipc::request("get_scene", json!({})).await?;
    let mut scene: Scene = serde_json::from_value(body).map_err(|e| format!("couldn't read the scene {} sent: {e}", ipc::app_name()))?;
    (scene.app, scene.app_name) = connected_app();
    Ok(scene)
}

/// the connected bridge's app id and name, what a scene it sends is marked with
pub fn connected_app() -> (Option<String>, Option<String>) {
    let state = lock();
    let known = |value: &String| Some(value.clone()).filter(|value| !value.is_empty());
    (known(&state.connected_application), known(&state.connected_name))
}

/// what the bridge reported after writing keyframes
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SceneWriteReport {
    #[serde(default)]
    pub missing_objects: Vec<String>,
    #[serde(default)]
    pub errors: Vec<WriteError>,
}

/// a channel (or a whole object, without a channel) the bridge couldn't write
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WriteError {
    pub object: String,
    #[serde(default)]
    pub channel: Option<String>,
    pub message: String,
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.channel {
            Some(channel) => write!(f, "{} {}: {}", self.object, channel, self.message),
            None => write!(f, "{}: {}", self.object, self.message),
        }
    }
}

/// writes curves ({object: [curve]}) through the bridge, the objects' animation replaced or added to
pub async fn write_scene_data(objects: serde_json::Value, clean_keyframes: bool) -> Result<SceneWriteReport, String> {
    let mode = if clean_keyframes {
        "replace"
    } else {
        "add"
    };
    let body = match ipc::request("write", json!({ "mode": mode, "objects": objects })).await {
        Ok(body) => body,
        // the bridge keeps writing after the wait is over
        Err(ipc::RequestError::Timeout(_)) => return Err(format!("{} didn't confirm the write within {}s, it may still be writing", ipc::app_name(), ipc::timeout("write").as_secs())),
        Err(error) => return Err(error.into()),
    };
    serde_json::from_value(body).map_err(|e| format!("couldn't read {}'s write report: {e}", ipc::app_name()))
}

/// a tab's scene came from one app and the connected bridge is another's, their ids don't match
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct AppChange {
    /// the apps' names, `Blender`
    pub saved: String,
    pub connected: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SceneDiff {
    /// the name of the app the new scene is from, for the review's text
    pub app: String,
    pub app_change: Option<AppChange>,
    pub missing_objects: Vec<String>, // Objects in saved but not in fresh
    pub new_objects: Vec<String>,     // Objects in fresh but not in saved
    pub missing_collections: Vec<String>,
    pub new_collections: Vec<String>,
}

impl SceneDiff {
    pub fn has_changes(&self) -> bool {
        self.app_change.is_some() || !self.missing_objects.is_empty() || !self.new_objects.is_empty() || !self.missing_collections.is_empty() || !self.new_collections.is_empty()
    }

    pub fn empty() -> Self {
        Self {
            app: String::new(),
            app_change: None,
            missing_objects: vec![],
            new_objects: vec![],
            missing_collections: vec![],
            new_collections: vec![],
        }
    }
}

/// the groups and objects `fresh` doesn't have that `saved` had, and the ones it has that `saved` didn't. a tab without a
/// scene has nothing to compare
pub fn compare_scene_data(saved: Option<&Scene>, fresh: &Scene) -> SceneDiff {
    let mut diff = SceneDiff::empty();
    diff.app = fresh.app_name.clone().or_else(|| fresh.app.clone()).unwrap_or_else(|| "3D app".to_string());
    let Some(saved) = saved else {
        return diff;
    };
    // shown by name, the app id for a scene saved before names were kept
    if let (Some(saved_app), Some(fresh_app)) = (&saved.app, &fresh.app) {
        if saved_app != fresh_app {
            let name = |scene: &Scene, app: &String| scene.app_name.clone().unwrap_or_else(|| app.clone());
            diff.app_change = Some(AppChange {
                saved: name(saved, saved_app),
                connected: name(fresh, fresh_app),
            });
        }
    }

    // groups match by id, by name when the tab's scene was saved before groups had ids
    let by_name = saved.object_groups.iter().any(|group| group.id.is_empty());
    let key = |group: &ObjectGroup| {
        if by_name {
            group.name.clone()
        } else {
            group.key().to_string()
        }
    };
    let saved_groups: HashSet<_> = saved.object_groups.iter().map(key).collect();
    let fresh_groups: HashSet<_> = fresh.object_groups.iter().map(key).collect();
    diff.missing_collections.extend(saved.object_groups.iter().filter(|group| !fresh_groups.contains(&key(group))).map(|group| group.name.clone()));
    diff.new_collections.extend(fresh.object_groups.iter().filter(|group| !saved_groups.contains(&key(group))).map(|group| group.name.clone()));

    // objects within the groups both have
    for saved_group in &saved.object_groups {
        if let Some(fresh_group) = fresh.object_groups.iter().find(|group| key(group) == key(saved_group)) {
            // objects match by id, they're shown by name
            let saved_objects: HashSet<_> = saved_group.objects.iter().map(|o| &o.id).collect();
            let fresh_objects: HashSet<_> = fresh_group.objects.iter().map(|o| &o.id).collect();
            diff.missing_objects.extend(saved_group.objects.iter().filter(|o| !fresh_objects.contains(&o.id)).map(|o| format!("{}/{}", saved_group.name, o.name)));
            diff.new_objects.extend(fresh_group.objects.iter().filter(|o| !saved_objects.contains(&o.id)).map(|o| format!("{}/{}", fresh_group.name, o.name)));
        }
    }

    diff
}

// the review of scene changes, for the tab on screen: Blender's scene differed from the tab's when it went live

/// what differs between the scene the tab on screen has and the one waiting for review
#[tauri::command]
pub async fn check_scene_changes() -> Result<SceneDiff, String> {
    let scenes = {
        let mut state = lock();
        let instance = state.active_mut();
        if !instance.execution_paused {
            return Err("No validation pending".to_string());
        }
        match &instance.pending_scene {
            Some(pending) => Ok((instance.scene.clone(), pending.clone())),
            // nothing to review, it runs again
            None => {
                instance.execution_paused = false;
                Err(instance.id.clone())
            }
        }
    };

    match scenes {
        Ok((saved, pending)) => Ok(compare_scene_data(saved.as_ref(), &pending)),
        Err(id) => {
            update_state();
            run_instance(id, true).await;
            Err("No pending scene data".to_string())
        }
    }
}

/// the tab on screen takes the scene waiting for review and runs again
#[tauri::command]
pub async fn accept_scene_changes() -> Result<(), String> {
    let (id, accepted) = {
        let mut state = lock();
        let instance = state.active_mut();
        let accepted = match instance.pending_scene.take() {
            Some(scene) => {
                instance.scene = Some(scene);
                true
            }
            None => false,
        };
        instance.execution_paused = false;
        (instance.id.clone(), accepted)
    };

    update_state();
    run_instance(id, true).await;
    if accepted {
        Ok(())
    } else {
        Err("No pending scene data".to_string())
    }
}

/// drops the scene waiting for review, the tab on screen stays paused with the scene it had
#[tauri::command]
pub fn reject_scene_changes() -> Result<(), String> {
    lock().active_mut().pending_scene = None;
    update_state();
    Ok(())
}
