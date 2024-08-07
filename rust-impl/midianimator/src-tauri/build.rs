use std::fs;
use std::path::Path;

use regex::Regex;
use walkdir::WalkDir;

fn main() {
    println!("cargo:warning=running build.rs");
    create_tauri_handlers();
    tauri_build::build();
}

fn create_tauri_handlers() {
    let out_path = Path::new("src/auto_commands.rs");
    
    println!("cargo:warning=creating tauri handlers in {:?}", out_path);

    let mut commands: Vec<String> = Vec::new();
    // god this regex was a pain to write
    let re = Regex::new(r"(?s)#\[(tauri::)?command\].*?\bfn\s+(\w+)").unwrap();
    
    // walk through all the files in the src directory
    for entry in WalkDir::new("src").into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().map_or(false, |ext| ext == "rs") {
            let content = fs::read_to_string(entry.path()).unwrap();
            
            // tell cargo to rerun the build script if any of the files change, i think it was caching the file results
            println!("cargo:rerun-if-changed={}", entry.path().display());
            
            for capture in re.captures_iter(&content) {
                if let Some(command_name) = capture.get(2) {
                    // format: crate_name::path::command_name
                    let path = entry.path().strip_prefix("src").unwrap().to_str().unwrap().replace("/mod", "").replace("/", "::").replace(".rs", "");
                    println!("cargo:warning=found path: {}", entry.path().to_str().unwrap());
                    println!("cargo:warning=found command: {}", command_name.as_str());
                    if path != "main" {  // sorry, main.rs, you're not allowed to have commands
                        commands.push(format!("crate::{}::{}", path, command_name.as_str()));
                    }
                    // commands.push(command_name.as_str().to_string());
                }
            }
        }
    }

    println!("cargo:warning=found commands: {:?}", commands);
    println!("cargo:warning=creating tauri handlers in {:?}", out_path);
    
    /* example generated code:
     *  pub fn get_cmds() -> impl Fn(tauri::Invoke<tauri::Wry>) + Send + Sync + 'static {
     *      return tauri::generate_handler![
     *          crate::command::test
     *      ];
     *  }
     */

    let handler = format!("tauri::generate_handler![{}]",commands.join(", "));

    let generated_code = format!("/* WARNING: This file is auto-generated by build.rs */
/* DO NOT MODIFY THIS FILE DIRECTLY (your changes will not be saved!)    */
/* also, this type annotation was so annoying to write */
pub fn get_cmds() -> impl Fn(tauri::Invoke<tauri::Wry>) + Send + Sync + 'static {{
    return {};
}}", handler);

    // check if the file is identical to the generated code
    let current_code = fs::read_to_string(&out_path).unwrap_or("".to_string());
    if current_code == generated_code {
        println!("cargo:warning=generated code is identical to current code, skipping write");
        return;
    }
    
    fs::write(out_path, generated_code).expect("failed to write auto_command.rs");

    println!("cargo:warning=written generated code to src/auto_command.rs");
    
    println!("cargo:rerun-if-changed={}", out_path.display());

}
