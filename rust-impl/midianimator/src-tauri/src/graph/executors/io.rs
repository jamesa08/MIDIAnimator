// typed inputs and outputs for node executors
//
// executors get their inputs as `Inputs` and return a `NodeResult`. the getters deserialize straight
// into the Rust types (`Vec<MIDITrack>`, `ObjectMap`, ...) and turn a missing or mistyped input into an
// error message instead of a panic. the executor loop stores that message on the node as `motionkeys_error`.
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::ops::Deref;

/// the key a failed node's results hold the error message under
pub const ERROR_KEY: &str = "motionkeys_error";

/// what every node executor returns: its outputs, or an error message for the node
pub type NodeResult = Result<Outputs, String>;

/// the signature every node executor has, used by the generated node registry
pub type NodeFunction = fn(Inputs) -> NodeResult;

/// returns the error message stored in a node's results, if the node failed
pub fn node_error(results: &Value) -> Option<&str> {
    results.get(ERROR_KEY).and_then(|v| v.as_str())
}

// MARK: - Inputs

/// the inputs of a node, keyed by input id
#[derive(Debug, Clone, Default)]
pub struct Inputs(HashMap<String, Value>);

impl Inputs {
    /// a required input, errors if it's missing, null, or the wrong type
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, String> {
        self.opt(key)?.ok_or_else(|| format!("missing input '{}'", key))
    }

    /// an optional input, `None` if it's missing or null, errors if it's the wrong type
    pub fn opt<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, String> {
        match self.0.get(key) {
            None | Some(Value::Null) => Ok(None),
            // deserialize from a reference so big inputs (tracks, notes) don't get cloned first
            Some(value) => T::deserialize(value).map(Some).map_err(|e| format!("input '{}' has the wrong type: {}", key, e)),
        }
    }

    /// an optional input that falls back to the type's default when it's missing or null
    pub fn or_default<T: DeserializeOwned + Default>(&self, key: &str) -> Result<T, String> {
        Ok(self.opt(key)?.unwrap_or_default())
    }
}

impl From<HashMap<String, Value>> for Inputs {
    fn from(map: HashMap<String, Value>) -> Self {
        Self(map)
    }
}

impl<const N: usize> From<[(&str, Value); N]> for Inputs {
    fn from(pairs: [(&str, Value); N]) -> Self {
        Self(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
}

// MARK: - Outputs

/// the outputs of a node, keyed by output id
#[derive(Debug, Clone, Default)]
pub struct Outputs(HashMap<String, Value>);

impl Outputs {
    pub fn new() -> Self {
        Self::default()
    }

    /// serializes a value into an output
    pub fn set<T: Serialize + ?Sized>(&mut self, key: &str, value: &T) -> Result<(), String> {
        let value = serde_json::to_value(value).map_err(|e| format!("could not serialize output '{}': {}", key, e))?;
        self.0.insert(key.to_string(), value);
        Ok(())
    }

    /// consumes the outputs into the plain map stored in `executed_results`
    pub fn into_map(self) -> HashMap<String, Value> {
        self.0
    }
}

// read access for the executor loop and tests
impl Deref for Outputs {
    type Target = HashMap<String, Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
