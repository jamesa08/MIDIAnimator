pub fn get_logical_size(window: &tauri::Window) -> tauri::LogicalSize<u32> {
    let cur_monitor: tauri::Monitor = window.current_monitor().unwrap().unwrap();
    let s_factor: f64 = cur_monitor.scale_factor();
    let phys_size: &tauri::PhysicalSize<u32> = cur_monitor.size();
    let logical_size: tauri::LogicalSize<u32> = phys_size.to_logical(s_factor);
    
    return logical_size;
}