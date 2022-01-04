use std::collections::HashMap;
use std::collections::HashSet;
use std::iter;
use std::sync;
use std::sync::atomic::AtomicU32;
use std::time::{Duration, Instant};

use twox_hash::RandomXxHashBuilder64;
use wgpu_glyph::{ab_glyph, GlyphBrush, GlyphBrushBuilder, Section, Text};
use wgpu_glyph::{HorizontalAlign, Layout, VerticalAlign};

use crate::params::sync::{Subscriber, Synchronizer};
use crate::params::{Params, ParamsMeta};
use crate::ui::buffer_memory;
use crate::ui::controls::Controls;
use crate::ui::coords::{Coord2, UserVec2, Vec2};
use crate::ui::shapes::{self, ScreenMetrics};
use crate::ui::sprites;
use crate::ui::styling;
use crate::ui::widgets::{LabelPosition, Widget, WidgetId};
use crate::util::borrow_return::{Borrower, Owner};

use baseview::{EventStatus, Window, WindowHandler, WindowScalePolicy};
use iced_baseview::Size;
use iced_native::clipboard;
use iced_native::keyboard::Modifiers;
use iced_native::Event as IcedEvent;
use iced_native::{program, Debug};
use iced_wgpu::{wgpu, Backend, Renderer, Settings, Viewport};

const DRAG_FACTOR_NORMAL: f32 = 4.0;
const DRAG_FACTOR_SLOW: f32 = 0.7;

/// How often to query the host for parameter updates (and thus update the GUI).
const PARAM_SYNC_PER_SEC: f32 = 60.0;

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
    mouse_pos_norm: Coord2,
    // TODO: Change to distinguish Ctrl, Shift, Cmd, etc.
    modifier_active_ctrl: bool,
}

impl State {
    pub async fn new<'a>(
        window: &'a Window<'a>,
        size: baseview::Size,
        scaling: f64,
        meta: sync::Arc<ParamsMeta>,
        styling: &styling::Styling,
    ) -> Self {
        let widgets = styling::create_widgets(styling, meta);

        let (render_state, widgets) =
            RenderState::new(widgets, window, size, scaling, styling).await;

        Self {
            widgets,
            interactive_state: InteractiveState::Idle,
            render_state,
            mouse_pos_norm: Coord2::new(-1.0, -1.0),
            modifier_active_ctrl: false,
        }
    }
}

struct RenderState {
    program_state: program::State<Controls>,
    events: Vec<IcedEvent>,
    debug: Debug,

    viewport: Viewport,
    device: wgpu::Device,
    swap_chain: wgpu::SwapChain,
    surface: wgpu::Surface,
    format: wgpu::TextureFormat,
    staging_belt: wgpu::util::StagingBelt,
    queue: wgpu::Queue,
    sc_desc: wgpu::SwapChainDescriptor,

    cursor_position: iced_baseview::Point,
    resized: bool,
    logical_size: Size,
    modifiers: Modifiers,
    window_info: baseview::WindowInfo,
    // TODO: Consider removing logical_size, use window_info directly.
    // TODO: Replace ScreenMetrics with window info.
    screen_metrics: ScreenMetrics,

    renderer: Renderer,

    background: [f64; 3],
    background_sprite_index: Option<usize>,
    spritesheet: sprites::SpriteSheet,
    shapes: shapes::Shapes,
    glyph_brush: GlyphBrush<(), ab_glyph::FontArc, RandomXxHashBuilder64>,

    default_padding: Coord2,

    // Helpful for printing debug information.
    #[allow(dead_code)]
    debug_poller: Poller,

    #[allow(dead_code)]
    iters: AtomicU32,
    fps: u32,
}

impl RenderState {
    async fn new<'a>(
        mut widgets: Vec<Widget>,
        window: &'a Window<'a>,
        size: baseview::Size,
        scaling: f64,
        styling: &styling::Styling,
    ) -> (Self, WidgetMap) {
        let window_info = baseview::WindowInfo::from_logical_size(size, scaling);

        let viewport = Viewport::with_physical_size(
            iced_baseview::Size::new(
                window_info.physical_size().width,
                window_info.physical_size().height,
            ),
            window_info.scale(),
        );

        let screen_metrics = ScreenMetrics::new(
            window_info.physical_size().width,
            window_info.physical_size().height,
            window_info.scale(),
        );

        // Initialize wgpu
        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
        let surface = unsafe { instance.create_surface(window) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Request adapter");
        let (device, queue) = {
            adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: None,
                        features: wgpu::Features::empty(),
                        limits: wgpu::Limits::default(),
                    },
                    None, // Trace path
                )
                .await
                .expect("Request device")
        };
        let swapchain_format = adapter.get_swap_chain_preferred_format(&surface);

        /////////////////////////////////////////////////////////////////
        // Sprites
        /////////////////////////////////////////////////////////////////
        let stylesheet_base_filename = styling
            .stylesheet_image
            .as_ref()
            .cloned()
            .expect("Stylesheet image could not be loaded");

        // Go up one folder
        let assets_folder = {
            let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap();
            base.join("assets")
        };

        let filename = assets_folder.join(stylesheet_base_filename);
        log::info!("Sprite base filename: {:?}", filename);

        let mut spritesheet_builder = sprites::SpriteSheetBuilder::new(
            &device,
            &swapchain_format,
            &queue,
            filename.to_str().unwrap(),
        );

        // Add a background, if one is given.
        let background_sprite_index = if let styling::Background::Sprite {
            dest_rect,
            src_rect,
        } = &styling.background
        {
            Some(spritesheet_builder.add(sprites::SpriteBuilder {
                pos: UserVec2::Rel(Vec2 {
                    pos: [dest_rect.x1(), dest_rect.y1()],
                }),
                size: UserVec2::Rel(Vec2 {
                    pos: [dest_rect.width(), dest_rect.height()],
                }),
                src_px: sprites::SpriteSource {
                    src_rect: src_rect.pos,
                },
            }))
        } else {
            None
        };

        let mut widget_map = HashMap::new();
        let mut shapes_builder =
            shapes::ShapesBuilder::with_capacity(128, &device, &swapchain_format);
        for mut widget in widgets.drain(..) {
            widget.initialize(
                &screen_metrics,
                &mut spritesheet_builder,
                &mut shapes_builder,
            );
            widget_map.insert(widget.id, widget);
        }
        /////////////////////////////////////////////////////////////////
        // Shapes
        /////////////////////////////////////////////////////////////////

        let spritesheet = spritesheet_builder.build(&screen_metrics);
        let shapes = shapes_builder.build();

        ///////////////////////////

        let sc_desc = {
            let size = window_info.physical_size();
            wgpu::SwapChainDescriptor {
                usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
                format: swapchain_format,
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::Fifo,
            }
        };

        /////////////////////////////////////////////////////////////////
        // Text
        /////////////////////////////////////////////////////////////////
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
            styling::Background::Sprite { .. } => [1.0, 1.0, 1.0], // TODO set back to black
        };

        let mut debug = Debug::new();
        let mut renderer = Renderer::new(Backend::new(&device, Settings::default()));

        let controls = Controls::new();

        let program_state = program::State::new(
            controls,
            viewport.logical_size(),
            iced_baseview::Point::new(-1.0, -1.0),
            &mut renderer,
            &mut debug,
        );

        let inst = Self {
            program_state,
            events: Vec::with_capacity(128),
            debug,
            screen_metrics,

            viewport,
            device,
            swap_chain,
            surface,
            format: swapchain_format,
            queue,
            sc_desc,

            cursor_position: iced_baseview::Point::new(-1.0, -1.0),
            resized: false,
            logical_size: iced_baseview::Size::new(
                window_info.logical_size().width as f32,
                window_info.logical_size().height as f32,
            ),
            modifiers: Modifiers::default(),
            window_info,

            renderer,

            spritesheet,
            shapes,
            background: background_color,
            background_sprite_index,

            default_padding: Coord2::new(styling.padding.0, styling.padding.1),

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
        new_size: &baseview::PhySize,
        widgets: &mut WidgetMap,
        params: &Synchronizer,
    ) {
        // Recreate the swap chain with the new size
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
        self.screen_metrics = ScreenMetrics::new(
            new_size.width,
            new_size.height,
            self.screen_metrics.scale_factor,
        );
        self.update_all_widgets(widgets, params);
        for (_widget_id, widget) in widgets.iter_mut() {
            widget.on_resize(
                &self.screen_metrics,
                &mut self.spritesheet,
                &mut self.shapes,
                params,
            );
        }
    }

    fn update_all_widgets(&mut self, widgets: &mut WidgetMap, params: &Synchronizer) {
        for (_widget_id, widget) in widgets.iter_mut() {
            widget.update(
                &self.screen_metrics,
                &mut self.spritesheet,
                &mut self.shapes,
                params,
            );
        }
        if let Some(sprite_index) = self.background_sprite_index {
            self.spritesheet.update_sprite(
                sprite_index,
                &sprites::SpriteUpdate::default(),
                &self.screen_metrics,
            );
        }
    }

    fn update_widgets(
        &mut self,
        widgets: &mut WidgetMap,
        params: &Synchronizer,
        updates: &HashSet<WidgetId>,
    ) {
        for (widget_id, widget) in widgets.iter_mut() {
            if updates.contains(widget_id) {
                widget.update(
                    &self.screen_metrics,
                    &mut self.spritesheet,
                    &mut self.shapes,
                    params,
                );
            }
        }
    }

    fn update_widget(&mut self, widgets: &mut WidgetMap, params: &Synchronizer, id: &WidgetId) {
        if let Some(widget) = widgets.get_mut(id) {
            widget.update(
                &self.screen_metrics,
                &mut self.spritesheet,
                &mut self.shapes,
                params,
            );
        } else {
            log::warn!("update_widget: widget not found! id={:?}", id);
        }
    }

    async fn render(&mut self, widgets: &mut WidgetMap) {
        if self.resized {
            let size = self.window_info.physical_size();

            self.swap_chain = self.device.create_swap_chain(
                &self.surface,
                &wgpu::SwapChainDescriptor {
                    usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
                    format: self.format,
                    width: size.width,
                    height: size.height,
                    present_mode: wgpu::PresentMode::Mailbox,
                },
            );

            self.resized = false;
        }
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
                label: Some("Render Encoder"),
            });

        // Note: read wgpu docs before reordering any of these operations.
        buffer_memory::update(
            &self.device,
            &mut self.shapes.shapes,
            &mut self.shapes.bufmem,
            &mut self.staging_belt,
            &mut encoder,
        );
        buffer_memory::update(
            &self.device,
            &mut self.spritesheet.shapes,
            &mut self.spritesheet.bufmem,
            &mut self.staging_belt,
            &mut encoder,
        );

        {
            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
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
            let rpass = self.spritesheet.render(rpass);
            self.shapes.render(rpass);
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
                self.window_info.physical_size().width,
                self.window_info.physical_size().height,
            )
            .expect("Draw queued");

        // Now draw iced over the scene.
        let _mouse_interaction = self.renderer.backend_mut().draw(
            &self.device,
            &mut self.staging_belt,
            &mut encoder,
            &frame.view,
            &self.viewport,
            self.program_state.primitive(),
            &self.debug.overlay(),
        );

        self.staging_belt.finish();
        self.queue.submit(iter::once(encoder.finish()));

        let f = self.staging_belt.recall();
        async_std::task::spawn(f);
    }
}

pub fn main() {
    let _ =
        simplelog::SimpleLogger::init(simplelog::LevelFilter::Info, simplelog::Config::default())
            .unwrap();

    let styling = styling::load_default();

    // Logical size.
    let size = baseview::Size::new(styling.size.0 as f64, styling.size.1 as f64);

    let options = baseview::WindowOpenOptions {
        title: "Sunfish Synthesizer".into(),
        size,
        scale: WindowScalePolicy::SystemScaleFactor,
    };

    let scaling = match options.scale {
        WindowScalePolicy::ScaleFactor(scale) => scale,
        WindowScalePolicy::SystemScaleFactor => 1.0,
    };
    baseview::Window::open_blocking(options, move |window| {
        let sample_rate = 44100.0;

        // Create the parameters themselves.
        let params = Params::new(sample_rate);
        let meta = ParamsMeta::new();

        let mut synchronizer = Synchronizer::new(meta, params);
        let subscriber = synchronizer.subscriber();
        let mut params_owner = Owner::new(synchronizer);
        let mut subscriber_owner = Owner::new(subscriber);

        SynthGui::create(
            window,
            &styling,
            params_owner.borrow(),
            subscriber_owner.borrow(),
            size,
            scaling,
        )
        .expect("SynthGui: failed to create.")
    });
}

pub struct SynthGui {
    // GUI and rendering state.
    state: State,

    parameters: Borrower<Synchronizer>,
    subscriber: Borrower<Subscriber>,

    #[allow(dead_code)]
    meta: sync::Arc<ParamsMeta>,
    param_sync_poller: Poller,
    widgets_to_update: HashSet<WidgetId>,
    _ignore_next_resized_event: bool,
}

impl SynthGui {
    pub fn create(
        window: &Window<'_>,
        styling: &styling::Styling,
        parameters: Borrower<Synchronizer>,
        subscriber: Borrower<Subscriber>,
        size: baseview::Size,
        scaling: f64,
    ) -> Result<SynthGui, std::io::Error> {
        let meta = (parameters.grabbed.as_ref().unwrap()).meta.clone();
        let meta = sync::Arc::new(meta);
        let param_count = meta.count();

        let state = async_std::task::block_on(State::new(
            window,
            size,
            scaling,
            sync::Arc::clone(&meta),
            styling,
        ));
        let param_sync_duration = Duration::from_secs_f32(1.0 / PARAM_SYNC_PER_SEC);
        let mut synth_gui = SynthGui {
            state,

            parameters,
            subscriber,
            meta,
            param_sync_poller: Poller::new(param_sync_duration),
            widgets_to_update: HashSet::with_capacity(param_count),
            _ignore_next_resized_event: false,
        };
        synth_gui.synchronize_all_params();
        Ok(synth_gui)
    }

    fn render_sync(&mut self) {
        self.state
            .render_state
            .iters
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        async_std::task::block_on(self.state.render_state.render(&mut self.state.widgets));
    }

    /// Load all baseline parameters.
    fn synchronize_all_params(&mut self) {
        self.synchronize_params();
    }

    /// Returns true if any parameters need changing.
    fn synchronize_params(&mut self) -> bool {
        let mut any_changed = false;
        if let Ok(guard) = self.subscriber.changes.try_lock() {
            let changes = &(*guard);
            self.widgets_to_update.clear();
            for (updated_eparam, updated_value) in changes {
                any_changed = true;
                let widget_id = WidgetId::Bound {
                    eparam: *updated_eparam,
                };
                if let Some(widget) = self.state.widgets.get_mut(&widget_id) {
                    widget.value = *updated_value;
                }
                self.widgets_to_update.insert(widget_id);
            }
            if any_changed {
                self.state.render_state.update_widgets(
                    &mut self.state.widgets,
                    &self.parameters,
                    &self.widgets_to_update,
                );
            }
        }
        any_changed
    }

    fn update_param(&mut self, id: &WidgetId, val: f64) {
        let eparam = match id {
            WidgetId::Unspecified { .. } => return,
            WidgetId::Bound { eparam } => *eparam,
        };
        self.parameters.write_parameter(eparam, val);
    }

    fn refresh_widget(&mut self, id: &WidgetId) {
        if let Some(widget) = self.state.widgets.get_mut(id) {
            if let Some(new_value) = widget.on_drag_done() {
                self.update_param(id, new_value);
                self.state.render_state.update_widget(
                    &mut self.state.widgets,
                    &self.parameters,
                    id,
                );
            }
        }
    }
}

impl WindowHandler for SynthGui {
    fn on_frame(&mut self, _window: &mut baseview::Window) {
        if self.param_sync_poller.tick() {
            self.parameters.refresh_maybe();
            self.synchronize_params();
        };
        self.render_sync();
    }

    fn on_event(&mut self, _window: &mut baseview::Window, event: baseview::Event) -> EventStatus {
        match &event {
            baseview::Event::Mouse(e) => {
                match e {
                    baseview::MouseEvent::ButtonPressed(baseview::MouseButton::Left) => {
                        match self.state.interactive_state {
                            InteractiveState::Idle => {
                                let (x, y) =
                                    (self.state.mouse_pos_norm.x, self.state.mouse_pos_norm.y);
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
                            InteractiveState::Dragging { id, .. } => {
                                self.refresh_widget(&id);
                            }
                        }
                    }
                    baseview::MouseEvent::ButtonReleased(baseview::MouseButton::Left) => {
                        if let InteractiveState::Dragging { id, .. } = self.state.interactive_state
                        {
                            self.refresh_widget(&id);
                        }
                        self.state.interactive_state = InteractiveState::Idle;
                    }
                    baseview::MouseEvent::WheelScrolled(_scroll_delta) => {}

                    baseview::MouseEvent::CursorMoved { position } => {
                        // Grab relative position.
                        let scaling = self.state.render_state.window_info.scale();
                        let (x, y) = (
                            self.state
                                .render_state
                                .screen_metrics
                                .screen_x_to_norm((position.x * scaling) as f32),
                            self.state
                                .render_state
                                .screen_metrics
                                .screen_y_to_norm((position.y * scaling) as f32),
                        );
                        self.state.mouse_pos_norm.x = x;
                        self.state.mouse_pos_norm.y = y;
                        if let InteractiveState::Dragging { id, mouse } =
                            &mut self.state.interactive_state
                        {
                            let id = *id;
                            let (cursor_x, cursor_y) = (
                                self.state.render_state.screen_metrics.screen_x_to_norm(
                                    self.state.render_state.cursor_position.x * (scaling as f32),
                                ),
                                self.state.render_state.screen_metrics.screen_y_to_norm(
                                    self.state.render_state.cursor_position.y * (scaling as f32),
                                ),
                            );
                            mouse.pos.x = cursor_x;
                            mouse.pos.y = cursor_y;
                            let df = if self.state.modifier_active_ctrl {
                                DRAG_FACTOR_SLOW
                            } else {
                                DRAG_FACTOR_NORMAL
                            };
                            if let Some(widget) = self.state.widgets.get_mut(&id) {
                                let tentative_value = widget.on_dragging(mouse, &df);
                                self.update_param(&id, tentative_value);
                            }
                            self.state.render_state.update_widget(
                                &mut self.state.widgets,
                                &self.parameters,
                                &id,
                            );
                        }
                        self.state.render_state.cursor_position =
                            conversion::baseview_point_to_iced_baseview_point(position);
                    }
                    // TODO: CursorEntered, CursorLeft
                    _ => {}
                }
            }
            baseview::Event::Keyboard(_) => {}
            baseview::Event::Window(e) => {
                match e {
                    baseview::WindowEvent::Resized(window_info) => {
                        self.state.render_state.logical_size =
                            conversion::baseview_size_to_iced_baseview_size(
                                &window_info.logical_size(),
                            );
                        self.state.render_state.viewport = Viewport::with_physical_size(
                            Size::new(
                                window_info.physical_size().width,
                                window_info.physical_size().height,
                            ),
                            window_info.scale(),
                        );
                        self.state.render_state.window_info = *window_info;
                        self.state.render_state.resized = true;
                        self.state.render_state.resize(
                            &window_info.physical_size(),
                            &mut self.state.widgets,
                            &self.parameters,
                        );
                    }
                    baseview::WindowEvent::WillClose => {
                        // TODO: Handle window close events.
                    }
                    _ => {}
                }
            }
        }

        iced_baseview::conversion::baseview_to_iced_events(
            event,
            &mut self.state.render_state.events,
            &mut self.state.render_state.modifiers,
        );
        for event in self.state.render_state.events.drain(..) {
            self.state.render_state.program_state.queue_event(event);
        }
        if !self.state.render_state.program_state.is_queue_empty() {
            // We update iced
            let _ = self.state.render_state.program_state.update(
                self.state.render_state.viewport.logical_size(),
                self.state.render_state.cursor_position,
                &mut self.state.render_state.renderer,
                &mut clipboard::Null,
                &mut self.state.render_state.debug,
            );
        }
        EventStatus::Captured
    }
}

mod conversion {

    pub fn baseview_size_to_iced_baseview_size(size: &baseview::Size) -> iced_baseview::Size {
        iced_baseview::Size::new(size.width as f32, size.height as f32)
    }

    pub fn baseview_point_to_iced_baseview_point(point: &baseview::Point) -> iced_baseview::Point {
        iced_baseview::Point::new(point.x as f32, point.y as f32)
    }
}
