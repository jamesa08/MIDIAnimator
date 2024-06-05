use serde_json::from_str;
pub fn get_keybind(file: &str, keybind: String) -> String {
    let json: serde_json::Value = from_str(&file).unwrap();
    let x = json[keybind]["key"].clone();
    return x.as_str().unwrap_or_default().to_string();
}