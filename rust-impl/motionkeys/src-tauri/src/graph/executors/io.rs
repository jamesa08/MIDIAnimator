// values passed between nodes, and the inputs and outputs of node executors
//
// nodes pass real Rust values to each other (`Val`), JSON only comes in where values do: from the UI (values set
// on a node) and out where they go: to the UI and IPC (the record of what each node got and gave). when a node
// asks for a type its input doesn't hold, the value is converted through JSON, and the run caches that so each
// value is only converted once per run.
//
// executors are written with typed arguments and the `node` attribute macro generates the glue that pulls them out
// of `Inputs`, see node_registry/src/lib.rs
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// the key a failed node's results hold the error message under
pub const ERROR_KEY: &str = "motionkeys_error";

/// the key a failed node's results hold its bad inputs under, input id to what's wrong with it. the UI marks the
/// connection into each one
pub const BAD_INPUTS_KEY: &str = "motionkeys_bad_inputs";

/// what every node executor returns: its outputs, or an error message for the node
pub type NodeResult = Result<Outputs, String>;

/// the signature every node executor has (the one `#[node]` generates), used by the generated node registry
pub type NodeFunction = fn(&Inputs) -> NodeResult;

/// returns the error message stored in a node's recorded results, if the node failed
pub fn node_error(results: &Value) -> Option<&str> {
    results.get(ERROR_KEY).and_then(|v| v.as_str())
}

// MARK: - Node Data

/// a type nodes can pass to each other. `split` gives the items of a list, for loops
pub trait NodeData: Any + Send + Sync {
    fn to_json(&self) -> Value;
    fn as_any(&self) -> &dyn Any;
    fn split(&self) -> Option<Vec<Val>> {
        None
    }
}

/// implements `NodeData` for types that aren't lists
#[macro_export]
macro_rules! node_data {
    ($($t:ty),* $(,)?) => {
        $(impl $crate::graph::executors::io::NodeData for $t {
            fn to_json(&self) -> serde_json::Value {
                serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        })*
    };
}

node_data!(String, f64, u8, u32, i64, bool, serde_json::Map<String, Value>);

// the types nodes pass around
use crate::midi::{MIDIEvent, MIDINote, MIDITrack};
use crate::scene_generics::{AnimCurve, KeyframePoint, Object, ObjectGroup, Scene};
use crate::utils::animation::{AnimationGenerator, BlendKeyframe, CurveKeys, NoteTarget, ObjectMap};
node_data!(MIDINote, MIDIEvent, MIDITrack, Scene, ObjectGroup, Object, AnimCurve, KeyframePoint, BlendKeyframe, AnimationGenerator, ObjectMap, NoteTarget, CurveKeys);

impl NodeData for Value {
    fn to_json(&self) -> Value {
        self.clone()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn split(&self) -> Option<Vec<Val>> {
        self.as_array().map(|items| items.iter().map(|v| Val::json(v.clone())).collect())
    }
}

impl<T: NodeData + Serialize + Clone> NodeData for Vec<T> {
    fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn split(&self) -> Option<Vec<Val>> {
        Some(self.iter().map(|item| Val::new(item.clone())).collect())
    }
}

impl<T: NodeData + Serialize> NodeData for Option<T> {
    fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<K: Serialize + Send + Sync + 'static, V: Serialize + Send + Sync + 'static> NodeData for HashMap<K, V> {
    fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<K: Serialize + Send + Sync + 'static, V: Serialize + Send + Sync + 'static> NodeData for BTreeMap<K, V> {
    fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// MARK: - Val

/// a value on a node's input or output, cheap to clone (shared)
#[derive(Clone)]
pub struct Val(Arc<Repr>);

enum Repr {
    Typed(Box<dyn NodeData>),
    /// JSON from the UI (a value set on a node) or a test
    Json(Value),
    /// a list of values collected by a loop
    List(Vec<Val>),
}

impl Val {
    pub fn new<T: NodeData>(value: T) -> Self {
        Self(Arc::new(Repr::Typed(Box::new(value))))
    }

    pub fn json(value: Value) -> Self {
        Self(Arc::new(Repr::Json(value)))
    }

    pub fn list(items: Vec<Val>) -> Self {
        Self(Arc::new(Repr::List(items)))
    }

    /// the value as `T` if that's what it holds
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        match &*self.0 {
            Repr::Typed(value) => value.as_any().downcast_ref(),
            Repr::Json(value) => (value as &dyn Any).downcast_ref(),
            Repr::List(_) => None,
        }
    }

    /// the value as `T`, for values `Inputs` already converted
    pub fn get<T: 'static>(&self) -> &T {
        self.downcast_ref().expect("input was converted to this type")
    }

    /// the value as JSON, for the UI and IPC
    pub fn to_json(&self) -> Value {
        match &*self.0 {
            Repr::Typed(value) => value.to_json(),
            Repr::Json(value) => value.clone(),
            Repr::List(items) => Value::Array(items.iter().map(Val::to_json).collect()),
        }
    }

    /// the items of a list, `None` if it isn't one
    pub fn items(&self) -> Option<Vec<Val>> {
        match &*self.0 {
            Repr::Typed(value) => value.split(),
            Repr::Json(value) => value.as_array().map(|items| items.iter().map(|v| Val::json(v.clone())).collect()),
            Repr::List(items) => Some(items.clone()),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(&*self.0, Repr::Json(Value::Null))
    }

    /// true if both are the same value (not just equal). a list is made again every run for a multi input, two
    /// lists are the same when their items are
    pub fn same(&self, other: &Val) -> bool {
        if Arc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        match (&*self.0, &*other.0) {
            (Repr::List(a), Repr::List(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.same(b)),
            _ => false,
        }
    }

    /// identifies the value while it's alive, for caches keyed by value
    pub(crate) fn address(&self) -> usize {
        Arc::as_ptr(&self.0) as *const () as usize
    }

    /// this value as `T`, converting through JSON when it holds something else
    fn convert<T: NodeData + DeserializeOwned>(&self) -> Result<Val, String> {
        if self.downcast_ref::<T>().is_some() {
            return Ok(self.clone());
        }
        let converted = match &*self.0 {
            // deserialize from a reference so big values (tracks, notes) don't get copied first
            Repr::Json(value) => T::deserialize(value),
            _ => T::deserialize(self.to_json()),
        };
        converted.map(Val::new).map_err(|e| e.to_string())
    }
}

impl std::fmt::Debug for Val {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_json())
    }
}

// MARK: - Conversion Cache

/// values converted to another type during one run, keyed by the value and the type it was converted to.
/// the source is kept alive with its conversion so its address can't be reused by another value
#[derive(Default)]
pub struct Conversions(RefCell<HashMap<(usize, TypeId), (Val, Val)>>);

impl Conversions {
    fn convert<T: NodeData + DeserializeOwned>(&self, value: &Val) -> Result<Val, String> {
        if value.downcast_ref::<T>().is_some() {
            return Ok(value.clone());
        }
        let key = (value.address(), TypeId::of::<T>());
        if let Some((_, converted)) = self.0.borrow().get(&key) {
            return Ok(converted.clone());
        }
        let converted = value.convert::<T>()?;
        self.0.borrow_mut().insert(key, (value.clone(), converted.clone()));
        Ok(converted)
    }
}

// MARK: - Inputs

/// the inputs of a node, keyed by input id
#[derive(Default)]
pub struct Inputs<'a> {
    values: Vec<(Arc<str>, Val)>,
    conversions: Option<&'a Conversions>,
    /// inputs that couldn't be converted to the type the node asked for
    wrong_type: RefCell<Vec<Arc<str>>>,
    /// a required input wasn't there
    missing: Cell<bool>,
}

impl<'a> Inputs<'a> {
    pub fn new(values: Vec<(Arc<str>, Val)>, conversions: &'a Conversions) -> Self {
        Self {
            values,
            conversions: Some(conversions),
            wrong_type: RefCell::default(),
            missing: Cell::default(),
        }
    }

    /// true if the node asked for a required input it didn't get
    pub fn missing(&self) -> bool {
        self.missing.get()
    }

    /// the inputs that had the wrong type for what the node asked for
    pub fn wrong_type(&self) -> Vec<Arc<str>> {
        self.wrong_type.borrow().clone()
    }

    /// an input as it came in, `None` if it's missing or null
    pub fn val(&self, key: &str) -> Option<&Val> {
        self.values.iter().find(|(k, _)| &**k == key).map(|(_, v)| v).filter(|v| !v.is_null())
    }

    /// a required input as `T`, errors if it's missing, null, or the wrong type
    pub fn value<T: NodeData + DeserializeOwned>(&self, key: &str) -> Result<Val, String> {
        self.value_opt::<T>(key)?.ok_or_else(|| {
            self.missing.set(true);
            format!("missing input '{}'", key)
        })
    }

    /// an optional input as `T`, `None` if it's missing or null, errors if it's the wrong type
    pub fn value_opt<T: NodeData + DeserializeOwned>(&self, key: &str) -> Result<Option<Val>, String> {
        let Some((name, value)) = self.values.iter().find(|(k, v)| &**k == key && !v.is_null()) else {
            return Ok(None);
        };
        let converted = match self.conversions {
            Some(conversions) => conversions.convert::<T>(value),
            None => value.convert::<T>(),
        };
        converted.map(Some).map_err(|e| {
            self.wrong_type.borrow_mut().push(name.clone());
            format!("input '{}' has the wrong type: {}", key, e)
        })
    }

    /// the numbered inputs of a `Dyn<T>` input in order, e.g. `object_maps_0`, `object_maps_1`, ... for `object_maps`.
    /// null ones are skipped
    pub fn dynamic<T: NodeData + DeserializeOwned>(&self, base: &str) -> Result<Vec<Val>, String> {
        let mut keys: Vec<(usize, &str)> = self.values.iter().filter_map(|(k, _)| crate::graph::model::dyn_index(base, k).map(|i| (i, &**k))).collect();
        keys.sort_unstable();
        let mut values = Vec::new();
        for (_, key) in keys {
            if let Some(value) = self.value_opt::<T>(key)? {
                values.push(value);
            }
        }
        Ok(values)
    }

    /// the inputs as JSON, for the record
    pub fn to_json(&self) -> Value {
        Value::Object(self.values.iter().map(|(k, v)| (k.to_string(), v.to_json())).collect())
    }
}

impl From<HashMap<String, Value>> for Inputs<'_> {
    fn from(map: HashMap<String, Value>) -> Self {
        Self {
            values: map.into_iter().map(|(k, v)| (Arc::from(k), Val::json(v))).collect(),
            conversions: None,
            wrong_type: RefCell::default(),
            missing: Cell::default(),
        }
    }
}

impl<const N: usize> From<[(&str, Value); N]> for Inputs<'_> {
    fn from(pairs: [(&str, Value); N]) -> Self {
        Self {
            values: pairs.into_iter().map(|(k, v)| (Arc::from(k), Val::json(v))).collect(),
            conversions: None,
            wrong_type: RefCell::default(),
            missing: Cell::default(),
        }
    }
}

// MARK: - Outputs

/// the outputs of a node, keyed by output id. shared, so passing them around doesn't copy them
#[derive(Default, Clone, Debug)]
pub struct Outputs(Arc<Vec<(Arc<str>, Val)>>);

impl Outputs {
    pub fn new() -> Self {
        Self::default()
    }

    /// sets an output
    pub fn set<T: NodeData>(&mut self, key: &str, value: T) {
        self.set_val(key, Val::new(value));
    }

    /// sets an output to a value that's already a `Val` (passed through from an input)
    pub fn set_val(&mut self, key: &str, value: Val) {
        let values = Arc::make_mut(&mut self.0);
        match values.iter_mut().find(|(k, _)| &**k == key) {
            Some(slot) => slot.1 = value,
            None => values.push((Arc::from(key), value)),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Val> {
        self.0.iter().find(|(k, _)| &**k == key).map(|(_, v)| v)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Val)> {
        self.0.iter().map(|(k, v)| (&**k, v))
    }

    /// the outputs as JSON, for the record (and tests)
    pub fn to_json(&self) -> Value {
        Value::Object(self.0.iter().map(|(k, v)| (k.to_string(), v.to_json())).collect())
    }
}
