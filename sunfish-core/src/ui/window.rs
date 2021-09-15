use std::collections::HashMap;
use std::sync;
use std::sync::atomic::AtomicU32;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use twox_hash::RandomXxHashBuilder64;
use wgpu;
use wgpu_glyph::{ab_glyph, GlyphBrush, GlyphBrushBuilder, Section, Text};
use wgpu_glyph::{HorizontalAlign, Layout, VerticalAlign};
use winit::{
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

use crate::modulation;
use crate::params;
use crate::params::deltas;
use crate::params::NormalizedParams;
use crate::params::SunfishParams;
use crate::swarc;
use crate::ui::coords::{Coord2, UserVec2, Vec2};
use crate::ui::shapes;
use crate::ui::sprites;
use crate::ui::styling;
use crate::ui::widgets::{LabelPosition, Widget, WidgetId};
use crate::util::borrow_return::{Borrower, Owner};

const DRAG_FACTOR_NORMAL: f32 = 4.0;
const DRAG_FACTOR_SLOW: f32 = 0.7;
const TICK_PER_SEC: f32 = 60.0; // 240.0;

/// How often to query the host for parameter updates (and thus update the GUI).
const PARAM_SYNC_PER_SEC: f32 = 10.0;

type WidgetMap = HashMap<WidgetId, Widget>;

/// Current, active GUI state (i.e. dragging something).
#[derive(PartialEq, Debug, Clone)]
pub enum InteractiveState {
    Idle,
    Dragging {
        id: WidgetId,
        mouse: ActiveMouseState,
    },
}

struct Poller {
    pub duration: Duration,
    last_tick: Instant,
}

impl Poller {
    fn new(duration: Duration) -> Self {
        Poller {
            duration,
            last_tick: Instant::now() - duration,
        }
    }

    #[allow(dead_code)]
    fn tick(&mut self) -> bool {
        let now = Instant::now();
        if now - self.last_tick >= self.duration {
            self.last_tick = now;
            return true;
        }
        false
    }
}

/// When Dragging, captures mouse x, y and
/// widget-relative coords.
#[derive(PartialEq, Debug, Clone)]
pub struct ActiveMouseState {
    /// Starting mouse position.
    pub start: Coord2,
    /// Current mouse position.
    pub pos: Coord2,
}

impl std::default::Default for ActiveMouseState {
    fn default() -> Self {
        Self {
            start: Coord2::new(0.0, 0.0),
            pos: Coord2::new(0.0, 0.0),
        }
    }
}

struct State {
    widgets: WidgetMap,
    render_state: RenderState,
    interactive_state: InteractiveState,
    mouse_pos: Coord2,
    render_poller: Poller,
    // TODO: Change to distinguish Ctrl, Shift, Cmd, etc.
    modifier_active_ctrl: bool,
}

impl State {
    pub async fn new(
        meta: sync::Arc<params::SunfishParamsMeta>,
        window: &Window,
        styling: &styling::Styling,
    ) -> Self {
        let widgets = styling::create_widgets(styling, meta);

        //log::info!("Loaded widgets: {:?}", widgets);
        let (render_state, widgets) = RenderState::new(window, widgets, styling).await;

        let tick_duration = Duration::from_secs_f32(1.0 / TICK_PER_SEC);
        Self {
            widgets,
            interactive_state: InteractiveState::Idle,
            render_poller: Poller::new(tick_duration),
            render_state,
            mouse_pos: Coord2::new(0.0, 0.0),
            modifier_active_ctrl: false,
        }
    }
}

struct RenderState {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    sc_desc: wgpu::SwapChainDescriptor,
    swap_chain: wgpu::SwapChain,
    last_position: winit::dpi::PhysicalPosition<f64>,
    screen_metrics: shapes::ScreenMetrics,
    aspect_ratio: f32,

    background: [f64; 3],
    spritesheet: sprites::SpriteSheet,
    bound_spritesheet: sprites::BoundSpriteSheet,
    shapes: shapes::Shapes,
    bound_shapes: shapes::BoundShapes,
    glyph_brush: GlyphBrush<(), ab_glyph::FontArc, RandomXxHashBuilder64>,
    staging_belt: wgpu::util::StagingBelt,

    default_padding: Coord2,

    // Helpful for printing debug information.
    #[allow(dead_code)]
    debug_poller: Poller,

    #[allow(dead_code)]
    iters: AtomicU32,
    fps: u32,
}

impl RenderState {
    async fn new(
        window: &Window,
        mut widgets: Vec<Widget>,
        styling: &styling::Styling,
    ) -> (Self, WidgetMap) {
        let swapchain_format = wgpu::TextureFormat::Bgra8UnormSrgb;

        let size = window.inner_size();

        let screen_metrics =
            shapes::ScreenMetrics::new(size.width, size.height, window.scale_factor());

        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::Default,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropiate adapter");

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    shader_validation: true,
                },
                None,
            )
            .await
            .expect("Failed to create device");

        /////////////////////////////////////////////////////////////////
        // Sprites
        /////////////////////////////////////////////////////////////////
        let spritesheet_base_filename = styling
            .spritesheet
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "synthsheet.png".to_string());

        // Go up one folder
        let assets_folder = {
            let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap();
            base.join("assets")
        };

        let filename = assets_folder.join(spritesheet_base_filename);
        log::info!("Sprite base filename: {:?}", filename);

        let mut spritesheet =
            sprites::SpriteSheet::new(&device, &queue, filename.to_str().unwrap());

        // Add a background, if one is given.
        if let styling::Background::Sprite {
            dest_rect,
            src_rect,
        } = &styling.background
        {
            spritesheet.add(sprites::Sprite {
                pos: UserVec2::Rel(Vec2 {
                    pos: [dest_rect.x1(), dest_rect.y1()],
                }),
                size: UserVec2::Rel(Vec2 {
                    pos: [dest_rect.width(), dest_rect.height()],
                }),
                src_px: sprites::SpriteSource {
                    src_rect: src_rect.pos,
                },
            });
        }
        // add all widgets
        let mut widget_map = HashMap::new();
        let mut shapes = shapes::Shapes::with_capacity(widgets.len());
        for mut widget in widgets.drain(..) {
            widget.initialize(&screen_metrics, &mut spritesheet, &mut shapes);
            widget_map.insert(widget.id, widget);
        }

        let bound_spritesheet = sprites::BoundSpriteSheet::new(
            &device,
            &swapchain_format,
            &spritesheet,
            &screen_metrics,
        );

        /////////////////////////////////////////////////////////////////
        // Shapes
        /////////////////////////////////////////////////////////////////

        let bound_shapes = shapes::BoundShapes::new(&device, &swapchain_format, &shapes);

        ///////////////////////////

        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Mailbox,
        };

        /////////////////////////////////////////////////////////////////
        // Text
        /////////////////////////////////////////////////////////////////
        // let font = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        //     .parent()
        //     .unwrap()
        //     .join("/assets/fonts/BluuNext-Bold.otf");
        // let font_bytes = include_bytes!("Inconsolata-Regular.ttf");
        //let font_bytes = std::fs::read(font).unwrap();
        //
        //let font_bytes = include_bytes!("../../../../assets/fonts/Inconsolata-Regular.ttf");
        // TODO: read from file instead.
        //let font_bytes = include_bytes!("../../../assets/fonts/BluuNext-Bold.otf");
        let font_bytes = include_bytes!("../../../assets/fonts/Oswald-Medium.ttf");
        let active_font = ab_glyph::FontArc::try_from_slice(font_bytes).unwrap();
        // Create staging belt and a local pool
        let staging_belt = wgpu::util::StagingBelt::new(1024);
        let glyph_brush =
            GlyphBrushBuilder::using_font(active_font).build(&device, swapchain_format);

        let swap_chain = device.create_swap_chain(&surface, &sc_desc);
        let background_color = match &styling.background {
            styling::Background::Solid { color } => {
                [color.r as f64, color.g as f64, color.b as f64]
            }
            styling::Background::Sprite { .. } => [0.0, 0.0, 0.0],
        };

        let aspect_ratio = screen_metrics.ratio;
        let inst = Self {
            surface,
            device,
            queue,

            sc_desc,
            swap_chain,
            screen_metrics,
            aspect_ratio,
            last_position: winit::dpi::PhysicalPosition::new(0.0, 0.0),

            background: background_color,

            default_padding: Coord2::new(styling.padding.0, styling.padding.1),

            spritesheet,
            bound_spritesheet,
            shapes,
            bound_shapes,
            glyph_brush,
            staging_belt,

            debug_poller: Poller::new(Duration::from_millis(1000)),
            iters: AtomicU32::new(0),
            fps: 0,
        };
        (inst, widget_map)
    }

    fn resize(
        &mut self,
        new_size: &winit::dpi::PhysicalSize<u32>,
        widgets: &mut WidgetMap,
        params: &swarc::ArcReader<SunfishParams>,
    ) {
        // Recreate the swap chain with the new size
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
        self.screen_metrics = shapes::ScreenMetrics::new(
            new_size.width,
            new_size.height,
            self.screen_metrics.scale_factor,
        );
        self.update_widgets(widgets, params);
        // TODO: Update only specific widgets.
        for (_widget_id, widget) in widgets.iter_mut() {
            widget.on_resize(
                &self.screen_metrics,
                &mut self.spritesheet,
                &mut self.shapes,
                params,
            );
        }
    }

    fn update_widgets(
        &mut self,
        widgets: &mut WidgetMap,
        params: &swarc::ArcReader<SunfishParams>,
    ) {
        // TODO: Update only specific widgets.
        for (_widget_id, widget) in widgets.iter_mut() {
            widget.update(
                &self.screen_metrics,
                &mut self.spritesheet,
                &mut self.shapes,
                params,
            );
        }
        self.bound_spritesheet
            .update(&self.device, &self.spritesheet, &self.screen_metrics);
    }

    async fn render(&mut self, widgets: &mut WidgetMap) {
        if self.debug_poller.tick() {
            self.fps = self.iters.swap(0, std::sync::atomic::Ordering::Relaxed);
        };
        let debug_text = format!("FPS: {}", self.fps);

        let frame = self
            .swap_chain
            .get_current_frame()
            .expect("Failed to acquire next swap chain texture")
            .output;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render encoder"),
            });

        // Note: read wgpu docs before reordering any of these operations.
        shapes::update(
            &self.device,
            &mut self.shapes,
            &mut self.bound_shapes,
            &mut self.staging_belt,
            &mut encoder,
        );
        {
            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.background[0],
                            g: self.background[1],
                            b: self.background[2],
                            a: 1.0,
                        }),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            // Render sprites first, then shapes.
            let rpass = sprites::render(&self.bound_spritesheet, rpass, &self.spritesheet);
            shapes::render(&self.bound_shapes, rpass);
        }

        for widget in widgets.values_mut() {
            let x1 = widget.rect.x1();
            let y1 = widget.rect.y1();
            let x2 = widget.rect.x2();
            let y2 = widget.rect.y2();

            let x_delta = x2 - x1;
            let y_delta = y2 - y1;
            let x_mid = x_delta / 2.0;
            let y_mid = y_delta / 2.0;

            let padding_x = self.default_padding.x;
            let padding_y = self.default_padding.y;

            let get_label_pos = |pos: &LabelPosition| match pos {
                LabelPosition::Left => (
                    x1 - padding_x,
                    y1 + y_mid,
                    HorizontalAlign::Right,
                    VerticalAlign::Center,
                ),
                LabelPosition::Right => (
                    x2 + padding_x,
                    y1 + y_mid,
                    HorizontalAlign::Left,
                    VerticalAlign::Center,
                ),
                LabelPosition::Middle => (
                    x1 + x_mid,
                    y1 + y_mid,
                    HorizontalAlign::Center,
                    VerticalAlign::Center,
                ),
                LabelPosition::Above => (
                    x1 + x_mid,
                    y1 + padding_y,
                    HorizontalAlign::Center,
                    VerticalAlign::Top,
                ),
                LabelPosition::Below { offset_relative } => (
                    x1 + x_mid,
                    y2 - padding_y + offset_relative.unwrap_or(0.0),
                    HorizontalAlign::Center,
                    VerticalAlign::Top,
                ),
                LabelPosition::Relative {
                    x,
                    y,
                    h_align,
                    v_align,
                } => (x1 + x, y1 + y, h_align.to_wgpu(), v_align.to_wgpu()),
            };

            widget.apply_to_texts(|text, color| {
                let (x, y, h_align, v_align) = get_label_pos(&text.pos);
                let screen_x = self.screen_metrics.norm_x_to_screen(x);
                let screen_y = self.screen_metrics.norm_y_to_screen(y);

                let layout = Layout::default_single_line()
                    .h_align(h_align)
                    .v_align(v_align);
                self.glyph_brush.queue(Section {
                    screen_position: (screen_x, screen_y),
                    // TODO: can add bounds: (x_bound, y_bound),
                    // TODO: avoid vec allocation
                    text: vec![Text::new(&text.value)
                        .with_color(color.to_array4())
                        .with_scale(text.scale * self.screen_metrics.width_f32)],
                    layout,
                    ..Default::default()
                });
            });
        }
        self.glyph_brush.queue(Section {
            screen_position: (5.0, 5.0),
            // TODO: can add bounds: (x_bound, y_bound),
            text: vec![Text::new(&debug_text)
                .with_color([1.0, 1.0, 1.0, 1.0])
                .with_scale(12.0)],
            ..Default::default()
        });

        // Draw queued text.
        self.glyph_brush
            .draw_queued(
                &self.device,
                &mut self.staging_belt,
                &mut encoder,
                &frame.view,
                self.screen_metrics.width_u32,
                self.screen_metrics.height_u32,
            )
            .expect("Draw queued");
        self.staging_belt.finish();

        self.queue.submit(Some(encoder.finish()));

        let f = self.staging_belt.recall();
        async_std::task::spawn(f);
    }
}

/// Create testing GUI.
async fn run(event_loop: EventLoop<()>, window: Window, styling: styling::Styling) {
    let sample_rate = 44100.0;
    let params = SunfishParams::new(sample_rate);
    let gui_params = params.clone();

    let param_set = modulation::ParamSet::new("main".to_string(), params);
    let gui_param_set = modulation::ParamSet::new("gui_param_set".to_string(), gui_params);
    let (gui_param_set_writer, gui_param_set_reader) = swarc::new(gui_param_set);
    let gui_param_reader = swarc::ArcReader::clone(&gui_param_set_reader.baseline);

    let baseline_param_reader = swarc::ArcReader::clone(&param_set.baseline);
    let modulated_param_reader = swarc::ArcReader::clone(&param_set.modulated);

    let baseline_deltas = deltas::Deltas::new(&param_set.meta);
    let from_gui_delta_tracker = baseline_deltas.create_tracker();
    let from_gui_deltas = sync::Arc::new(Mutex::new(baseline_deltas));
    let for_gui_deltas = sync::Arc::new(Mutex::new(deltas::Deltas::new(&param_set.meta)));

    let mut gui_param_set = Owner::new(gui_param_set_writer);

    let mut sg = SynthGui::create(
        &window,
        &styling,
        baseline_param_reader,
        modulated_param_reader,
        gui_param_set.borrow(),
        sync::Arc::clone(&from_gui_deltas),
        sync::Arc::clone(&for_gui_deltas),
    )
    .expect("SynthGui: failed to create.");

    async_std::task::spawn(async move {
        update_gui_params(
            param_set.meta.clone(),
            sync::Arc::clone(&from_gui_deltas),
            from_gui_delta_tracker,
            gui_param_reader,
            param_set.baseline_writer,
        )
        .await;
    });

    event_loop.run(move |event, _, control_flow| sg.receive_events(&window, event, control_flow));
}

/// For testing standalone GUI.
async fn update_gui_params(
    meta: params::SunfishParamsMeta,
    from_gui_deltas: sync::Arc<Mutex<deltas::Deltas>>,
    mut from_gui_delta_tracker: deltas::DeltaChangeTracker,
    params: swarc::ArcReader<params::SunfishParams>,
    mut params_writer: swarc::ArcWriter<params::SunfishParams>,
) {
    loop {
        // Grab the lock, check for updates.
        let mut any_changed = false;

        if let Ok(ref mut from_gui_deltas) = from_gui_deltas.try_lock() {
            if from_gui_deltas.any_changed() {
                // Update the cached changes.
                from_gui_delta_tracker.refresh_changed(&meta, &from_gui_deltas);
                from_gui_deltas.reset();
                any_changed = true;
            }
        }
        if any_changed {
            // Now see which parameters changed and update them.
            for changed in &from_gui_delta_tracker.changed_list_cached {
                let param_value = params.get_param_normalized(&meta, *changed).unwrap();

                params_writer
                    .update_param(&meta, *changed, param_value)
                    .unwrap();
            }
        }
        async_std::task::sleep(std::time::Duration::from_millis(1)).await;
    }
}

pub fn main() {
    let _ =
        simplelog::SimpleLogger::init(simplelog::LevelFilter::Info, simplelog::Config::default())
            .unwrap();
    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();

    let styling = styling::load_default();
    window.set_inner_size(winit::dpi::PhysicalSize::new(
        styling.size.0,
        styling.size.1,
    ));

    async_std::task::block_on(run(event_loop, window, styling));
}
pub struct SynthGui {
    // GUI and rendering state.
    state: State,

    // Views into the canonical params.
    baseline_param_reader: swarc::ArcReader<SunfishParams>,
    #[allow(dead_code)]
    modulated_param_reader: swarc::ArcReader<SunfishParams>,

    // Owned GUI parameters.
    gui_param_set: Borrower<swarc::ArcWriter<modulation::ParamSet>>,

    // Channels for receiving and propagating parameter updates.
    from_gui_deltas: sync::Arc<Mutex<deltas::Deltas>>,
    from_gui_deltas_pending: deltas::Deltas,
    from_gui_deltas_pending_tracker: deltas::DeltaChangeTracker,
    for_gui_deltas: sync::Arc<Mutex<deltas::Deltas>>,
    for_gui_deltas_tracker: deltas::DeltaChangeTracker,

    meta: sync::Arc<params::SunfishParamsMeta>,
    param_sync_poller: Poller,

    ignore_next_resized_event: bool,
}

impl SynthGui {
    pub fn create(
        window: &Window,
        styling: &styling::Styling,
        baseline_param_reader: swarc::ArcReader<SunfishParams>,
        modulated_param_reader: swarc::ArcReader<SunfishParams>,
        gui_param_set: Borrower<swarc::ArcWriter<modulation::ParamSet>>,
        from_gui_deltas: sync::Arc<Mutex<deltas::Deltas>>,
        for_gui_deltas: sync::Arc<Mutex<deltas::Deltas>>,
    ) -> Result<SynthGui, std::io::Error> {
        let meta = (gui_param_set.grabbed.as_ref().unwrap()).meta.clone();

        let from_gui_deltas_pending = deltas::Deltas::new(&meta);
        let from_gui_deltas_pending_tracker = from_gui_deltas_pending.create_tracker();

        let for_gui_deltas_tracker = for_gui_deltas.lock().unwrap().create_tracker();

        let meta = sync::Arc::new(meta);
        let state =
            async_std::task::block_on(State::new(sync::Arc::clone(&meta), &window, styling));
        let param_sync_duration = Duration::from_secs_f32(1.0 / PARAM_SYNC_PER_SEC);
        let mut synth_gui = SynthGui {
            state,

            gui_param_set,
            baseline_param_reader,
            modulated_param_reader,
            from_gui_deltas,
            from_gui_deltas_pending,
            from_gui_deltas_pending_tracker,
            for_gui_deltas,
            for_gui_deltas_tracker,
            meta,
            param_sync_poller: Poller::new(param_sync_duration),

            ignore_next_resized_event: false,
        };
        synth_gui.synchronize_all_params();
        Ok(synth_gui)
    }

    pub fn receive_events<'a>(
        &mut self,
        window: &Window,
        event: Event<'a, ()>,
        control_flow: &mut ControlFlow,
    ) {
        // TODO: Should we unconditionally render?
        if self.param_sync_poller.tick() {
            let trigger_render = self.synchronize_params();
            if trigger_render {
                self.render_sync();
            }
        }

        let next_tick = Instant::now() + self.state.render_poller.duration;
        *control_flow = ControlFlow::WaitUntil(next_tick);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::ScaleFactorChanged {
                    scale_factor,
                    new_inner_size,
                } => {
                    self.state.render_state.screen_metrics.scale_factor = scale_factor;
                    self.state.render_state.resize(
                        new_inner_size,
                        &mut self.state.widgets,
                        &self.baseline_param_reader,
                    );
                    window.request_redraw();
                }
                WindowEvent::ModifiersChanged(modifiers_state) => {
                    self.state.modifier_active_ctrl =
                        modifiers_state.intersects(winit::event::ModifiersState::CTRL);
                }
                WindowEvent::Resized(size) => {
                    // if !self.ignore_next_resized_event {
                    // Constrain the resize; TODO: how? look at the last window size?
                    let (new_width, new_height) = (size.width, size.height);
                    //   self.state.render_state.screen_metrics.constrain_resize(
                    //       size.width,
                    //       size.height,
                    //       self.state.render_state.aspect_ratio,
                    //   );

                    // let override_size = winit::dpi::PhysicalSize::new(new_width, new_height);
                    // window.set_inner_size(override_size);

                    self.state.render_state.resize(
                        &size,
                        //&override_size,
                        &mut self.state.widgets,
                        &self.baseline_param_reader,
                    );
                    window.request_redraw();
                    //self.ignore_next_resized_event = true;
                    // } else {
                    //     self.ignore_next_resized_event = false;
                    // }
                }
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::MouseInput {
                    state: input_state,
                    button,
                    ..
                } => match button {
                    MouseButton::Left => match self.state.interactive_state {
                        InteractiveState::Idle => {
                            if input_state == ElementState::Pressed {
                                let (x, y) = (self.state.mouse_pos.x, self.state.mouse_pos.y);
                                for (widget_id, widget) in self.state.widgets.iter_mut() {
                                    if widget.interactive && widget.in_bounds_rel(x, y) {
                                        let mouse = ActiveMouseState {
                                            pos: Coord2::new(x, y),
                                            start: Coord2::new(x, y),
                                        };
                                        let drag_factor = DRAG_FACTOR_NORMAL;
                                        widget.on_drag_start(&mouse, &drag_factor);
                                        self.state.interactive_state = InteractiveState::Dragging {
                                            id: *widget_id,
                                            mouse,
                                        };
                                        break;
                                    }
                                }
                            }
                        }
                        InteractiveState::Dragging { id, .. } => {
                            if let Some(widget) = self.state.widgets.get_mut(&id) {
                                if let Some(new_value) = widget.on_drag_done() {
                                    self.update_param(&id, new_value);
                                }
                            }
                            if input_state == ElementState::Released {
                                self.state.interactive_state = InteractiveState::Idle;
                            }
                        }
                    },
                    _ => {}
                },
                WindowEvent::CursorMoved { position, .. } => {
                    // Grab relative position.
                    let (x, y) = (
                        self.state
                            .render_state
                            .screen_metrics
                            .screen_x_to_norm(position.x as f32),
                        self.state
                            .render_state
                            .screen_metrics
                            .screen_y_to_norm(position.y as f32),
                    );

                    self.state.mouse_pos.x = x;
                    self.state.mouse_pos.y = y;
                    if let InteractiveState::Dragging { id, mouse } =
                        &mut self.state.interactive_state
                    {
                        mouse.pos.x = self.state.mouse_pos.x;
                        mouse.pos.y = self.state.mouse_pos.y;
                        let df = if self.state.modifier_active_ctrl {
                            DRAG_FACTOR_SLOW
                        } else {
                            DRAG_FACTOR_NORMAL
                        };
                        if let Some(widget) = self.state.widgets.get_mut(&id) {
                            let tentative_value = widget.on_dragging(&mouse, &df);
                            let id = id.clone();
                            self.update_param(&id, tentative_value);
                        }
                        self.state
                            .render_state
                            .update_widgets(&mut self.state.widgets, &self.baseline_param_reader);
                    }
                    self.state.render_state.last_position = position;
                    window.request_redraw();
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                self.render_sync();
                self.state
                    .render_state
                    .iters
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            Event::MainEventsCleared => {
                self.render_sync();
                self.state
                    .render_state
                    .iters
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            _ => {}
        }
    }

    fn render_sync(&mut self) {
        async_std::task::block_on(self.state.render_state.render(&mut self.state.widgets));
    }

    /// Load all baseline parameters.
    fn synchronize_all_params(&mut self) {
        if let Ok(ref mut for_gui_deltas) = self.for_gui_deltas.try_lock() {
            for_gui_deltas.set_all();
            self.for_gui_deltas_tracker
                .refresh_changed(&self.meta, &for_gui_deltas);
        }
        self.synchronize_params();
    }

    /// Returns true if any parameters need changing.
    fn synchronize_params(&mut self) -> bool {
        let mut any_changed = false;
        if let Ok(ref mut for_gui_deltas) = self.for_gui_deltas.try_lock() {
            if for_gui_deltas.any_changed() {
                self.for_gui_deltas_tracker
                    .refresh_changed(&self.meta, &for_gui_deltas);
                any_changed = true;
                for_gui_deltas.reset();
            }
        }
        if any_changed {
            for updated_eparam in &self.for_gui_deltas_tracker.changed_list_cached {
                let val = self
                    .baseline_param_reader
                    .get_param_normalized(&self.meta, *updated_eparam)
                    .unwrap_or(0.0);

                let widget_id = WidgetId::Bound {
                    eparam: *updated_eparam,
                };
                self.state.widgets.get_mut(&widget_id).map(|widget| {
                    widget.value = val;
                });
            }
            self.state
                .render_state
                .update_widgets(&mut self.state.widgets, &self.baseline_param_reader);
        }
        any_changed
    }

    fn update_param(&mut self, id: &WidgetId, val: f64) {
        let eparam = match id {
            WidgetId::Unspecified { .. } => return,
            WidgetId::Bound { eparam } => *eparam,
        };
        //  This should never fail; probably could streamline this.
        if let Some(gui_param_set) = &mut self.gui_param_set.grabbed {
            // First update the parameter.
            gui_param_set
                .baseline_writer
                .update_param(&self.meta, eparam, val)
                .unwrap();

            // Then the bitmask, if we can.
            // If we cannot acquire this lock, then store into
            // pending changes.
            // TODO: Factor out into helper?
            if let Ok(ref mut from_gui_deltas) = self.from_gui_deltas.try_lock() {
                if self.from_gui_deltas_pending.any_changed() {
                    self.from_gui_deltas_pending_tracker
                        .refresh_changed(&self.meta, &self.from_gui_deltas_pending);
                    for updated_eparam in &self.from_gui_deltas_pending_tracker.changed_list_cached
                    {
                        from_gui_deltas.set_changed(&self.meta, &updated_eparam);
                    }
                    self.from_gui_deltas_pending.reset();
                }
                from_gui_deltas.set_changed(&self.meta, &eparam);
            } else {
                // store into pending
                self.from_gui_deltas_pending
                    .set_changed(&self.meta, &eparam);
            }
        }
    }
}
