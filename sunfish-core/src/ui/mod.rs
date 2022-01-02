pub mod alignment;
pub mod buffer_memory;
pub mod buffers;
pub mod controls;
pub mod coords;
pub mod editor;
pub mod shape_util;
pub mod shapes;
pub mod sprites;
pub mod styling;
pub mod texture;
pub mod widgets;
pub mod window;

#[cfg(target_os = "macos")]
pub fn editor_supported() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn editor_supported() -> bool {
    false
}
