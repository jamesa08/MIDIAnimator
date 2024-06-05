#![allow(non_snake_case)]

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

pub mod command {
    pub mod event;
}