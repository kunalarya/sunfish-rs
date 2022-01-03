use std::os::raw::c_void;

use baseview::WindowScalePolicy;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use vst::editor::{Editor, KeyCode, KnobMode};

use crate::params::sync::{Subscriber, Synchronizer};
use crate::ui::styling;
use crate::ui::window;
use crate::util::borrow_return::Owner;

pub struct SunfishEditor {
    open: bool,

    parameters: Owner<Synchronizer>,
    subscriber: Owner<Subscriber>,
    /// Metadata/GUI layout.
    styling: styling::Styling,
}

impl SunfishEditor {
    pub fn new(parameters: Synchronizer, subscriber: Subscriber) -> SunfishEditor {
        let styling = styling::load_default();
        SunfishEditor {
            open: false,
            parameters: Owner::new(parameters),
            subscriber: Owner::new(subscriber),
            styling,
        }
    }
}

impl Editor for SunfishEditor {
    /// Get the size of the editor window.
    fn size(&self) -> (i32, i32) {
        self.styling.size
    }

    /// Get the coordinates of the editor window.
    fn position(&self) -> (i32, i32) {
        (100, 100)
    }

    /// Editor idle call. Called by host.
    fn idle(&mut self) {}

    /// Called when the editor window is closed.
    fn close(&mut self) {
        // we have to manually drop the objects we allocated.
        self.open = false;
    }

    /// Called when the editor window is opened. `window` is a platform dependent window pointer
    /// (e.g. `HWND` on Windows, `NSView` (64-bit Cocoa) on OSX, `Window` on X11/Linux).
    fn open(&mut self, parent: *mut c_void) -> bool {
        if self.open {
            return false;
        }
        log::info!("Sunfish: open, parent={:?}", parent);

        // TODO: Consolidate with standalone options.
        // Logical size.
        let size = baseview::Size::new(self.styling.size.0 as f64, self.styling.size.1 as f64);

        let options = baseview::WindowOpenOptions {
            title: "Sunfish Synthesizer".into(),
            size,
            scale: WindowScalePolicy::SystemScaleFactor,
        };

        let scaling = match options.scale {
            WindowScalePolicy::ScaleFactor(scale) => scale,
            WindowScalePolicy::SystemScaleFactor => 1.0,
        };

        let styling = self.styling.clone();
        let param_borrow = self.parameters.borrow();
        let subscriber_borrow = self.subscriber.borrow();

        baseview::Window::open_parented(&ParentWindow(parent), options, move |window| {
            window::SynthGui::create(
                window,
                &styling,
                param_borrow,
                subscriber_borrow,
                size,
                scaling,
            )
            .expect("Cannot create synth GUI")
        });
        true
    }

    /// Return whether the window is currently open.
    fn is_open(&mut self) -> bool {
        self.open
    }

    /// Set the knob mode for this editor (if supported by host).
    ///
    /// Return true if the knob mode was set.
    fn set_knob_mode(&mut self, _mode: KnobMode) -> bool {
        false
    }

    /// Recieve key up event. Return true if the key was used.
    fn key_up(&mut self, _keycode: KeyCode) -> bool {
        false
    }

    /// Receive key down event. Return true if the key was used.
    fn key_down(&mut self, _keycode: KeyCode) -> bool {
        false
    }
}

// Courtesy of OctaSine:
pub struct ParentWindow(pub *mut ::core::ffi::c_void);

unsafe impl HasRawWindowHandle for ParentWindow {
    #[cfg(target_os = "macos")]
    fn raw_window_handle(&self) -> RawWindowHandle {
        use raw_window_handle::macos::MacOSHandle;

        RawWindowHandle::MacOS(MacOSHandle {
            ns_view: self.0,
            ..MacOSHandle::empty()
        })
    }

    #[cfg(target_os = "windows")]
    fn raw_window_handle(&self) -> RawWindowHandle {
        use raw_window_handle::windows::WindowsHandle;

        RawWindowHandle::Windows(WindowsHandle {
            hwnd: self.0,
            ..WindowsHandle::empty()
        })
    }

    #[cfg(target_os = "linux")]
    fn raw_window_handle(&self) -> RawWindowHandle {
        use raw_window_handle::unix::XcbHandle;

        RawWindowHandle::Xcb(XcbHandle {
            window: self.0 as u32,
            ..XcbHandle::empty()
        })
    }
}
