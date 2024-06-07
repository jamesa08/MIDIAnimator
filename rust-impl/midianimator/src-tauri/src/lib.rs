#![allow(non_snake_case)]
#![allow(dead_code)]

pub mod utils;

pub mod structures { 
    pub mod midi; 
    pub mod ipc;
}
pub mod scene_generics;

pub mod build_scene;

pub mod ui {
    pub mod menu;
    pub mod keybinds;
}

pub mod api {
    pub mod event;
    pub mod ipc;
    pub mod midi;
}

pub mod auto_commands;