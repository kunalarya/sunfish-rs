pub mod alignment;
pub mod buffers;
pub mod coords;
pub mod editor;
pub mod packed_shapes;
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
