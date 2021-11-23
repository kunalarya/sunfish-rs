use std::os::raw::c_void;
use vst::editor::{Editor, KeyCode, KnobMode};

#[cfg(target_os = "macos")]
use winit::event_loop::EventSubscriber;
#[cfg(target_os = "macos")]
use winit::window::ChildWindow;
#[cfg(target_os = "macos")]
use winit::window::WindowBuilder;

use crate::params::sync::{Subscriber, Synchronizer};
use crate::ui::styling;
use crate::ui::window;
use crate::util::borrow_return::Owner;

#[cfg(target_os = "macos")]
use winit::platform::macos::{ParentHandle, WindowBuilderExtMacOS};

pub struct SunfishEditor {
    open: bool,
    #[cfg(target_os = "macos")]
    event_subscriber: Option<EventSubscriber>,

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
            #[cfg(target_os = "macos")]
            event_subscriber: None,
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
        #[cfg(target_os = "macos")]
        {
            self.event_subscriber = None;
        }
        self.open = false;
    }

    /// Called when the editor window is opened. `window` is a platform dependent window pointer
    /// (e.g. `HWND` on Windows, `NSView` (64-bit Cocoa) on OSX, `Window` on X11/Linux).
    #[cfg(target_os = "macos")]
    fn open(&mut self, window: *mut c_void) -> bool {
        log::info!("Sunfish: open, window={:?}", window);
        let window_result = WindowBuilder::new()
            .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
            .with_resizable(true)
            .with_decorations(true)
            .for_parent(ParentHandle::NsView(window));

        if let Ok(child_window) = window_result {
            let ChildWindow {
                window,
                mut event_subscriber,
                ..
            } = child_window;

            if let Ok(mut synth_gui) = window::SynthGui::create(
                &window,
                &self.styling,
                self.parameters.borrow(),
                self.subscriber.borrow(),
            ) {
                event_subscriber.receive_events(move |event, _, control_flow| {
                    // The event subscriber callback owns the GUI. As soon as it
                    // gets dropped, the GUI drops.
                    synth_gui.receive_events(&window, event, control_flow)
                });
            } else {
                log::error!("Error creating GUI!");
            }
            self.event_subscriber = Some(event_subscriber);
            self.open = true;
            true
        } else {
            log::info!("window not ok");
            false
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn open(&mut self, window: *mut c_void) -> bool {
        false
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
