use std::sync::Arc;

use serde::Deserialize;

use crate::params::sync::Synchronizer;
use crate::params::{EParam, ParamsMeta};
use crate::ui::alignment::{HorizontalAlign, VerticalAlign};
use crate::ui::coords::{Coord2, Rect};
use crate::ui::shapes;
use crate::ui::shapes::{Color, Polarity};
use crate::ui::sprites;
use crate::ui::window::ActiveMouseState;

const DEFAULT_TEXT_COLOR: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
};

#[allow(dead_code)]
const TOGGLE_OUTLINE_COLOR: [f32; 3] = [1.0, 0.0, 0.0];

#[derive(Copy, Clone, Debug)]
pub struct SpriteIndex(usize);

#[derive(Copy, Clone, Debug)]
pub struct ShapeIndex(usize);

#[derive(Copy, Clone, Debug, Deserialize)]
pub enum LabelPosition {
    Below {
        offset_relative: Option<f32>,
    },
    Above,
    Left,
    Right,
    Middle,
    Relative {
        x: f32,
        y: f32,
        h_align: HorizontalAlign,
        v_align: VerticalAlign,
    },
}

#[derive(Copy, Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub enum WidgetId {
    Unspecified { id: usize }, // Assign a unique ID as we use this as a hashmap key
    Bound { eparam: EParam },
}

impl WidgetId {
    pub fn as_string(&self) -> String {
        match self {
            Self::Unspecified { id } => format!("Unspecified ({})", id),
            Self::Bound { eparam } => eparam.as_string(true),
        }
    }
}

struct UpdateContext<'a> {
    #[allow(dead_code)]
    meta: &'a Arc<ParamsMeta>,
    params: &'a Synchronizer,
    id: &'a WidgetId,
    rect: &'a Rect,
    screen_metrics: &'a shapes::ScreenMetrics,
    #[allow(dead_code)]
    spritesheet: &'a mut sprites::SpriteSheet,
    shapes: &'a mut shapes::Shapes,
}

#[derive(Debug)]
pub struct Widget {
    meta: Arc<ParamsMeta>,
    pub id: WidgetId,
    pub rect: Rect,
    pub value: f64, // Normalized between 0.0 and 1.0
    pub baseline_value: Option<f64>,
    pub tentative_value: Option<f64>,
    pub wt: WidgetClass,
    pub interactive: bool,
}

impl Widget {
    pub fn new(
        meta: Arc<ParamsMeta>,
        id: WidgetId,
        rect: Rect,
        value: f64,
        wt: WidgetClass,
    ) -> Self {
        let interactive = !matches!(wt, WidgetClass::Panel(_));
        Self {
            meta,
            id,
            rect,
            value,
            baseline_value: None,
            tentative_value: None,
            wt,
            interactive,
        }
    }

    pub fn apply_to_texts<F: FnMut(&Text, &Color)>(&self, f: F) {
        match &self.wt {
            WidgetClass::Knob(knob) => knob.apply_to_texts(f),
            WidgetClass::Spinner(spinner) => spinner.apply_to_texts(f),
            WidgetClass::VSlider(_vslider) => { /* TODO */ }
            WidgetClass::Panel(_panel) => { /* TODO */ }
            WidgetClass::Toggle(toggle) => toggle.apply_to_texts(f),
        }
    }

    pub fn in_bounds_rel(&self, x: f32, y: f32) -> bool {
        self.rect.in_bounds(x, y)
    }

    pub fn on_drag_start(&mut self, mouse_state: &ActiveMouseState, drag_factor: &f32) -> f64 {
        self.baseline_value = Some(self.value);
        self.on_dragging(mouse_state, drag_factor)
    }

    pub fn on_dragging(&mut self, mouse_state: &ActiveMouseState, drag_factor: &f32) -> f64 {
        let baseline_value = self.baseline_value.unwrap_or(self.value);
        let tentative_value = match &mut self.wt {
            WidgetClass::Knob(knob) => knob.on_dragging(mouse_state, drag_factor, baseline_value),
            WidgetClass::VSlider(vslider) => vslider.on_dragging(&self.rect, mouse_state),
            WidgetClass::Spinner(spinner) => {
                spinner.on_dragging(mouse_state, drag_factor, baseline_value)
            }
            WidgetClass::Toggle(toggle) => toggle.on_dragging(baseline_value),
            WidgetClass::Panel(_) => 0.0,
        };
        self.tentative_value = Some(tentative_value);
        tentative_value
    }

    pub fn on_drag_done(&mut self) -> Option<f64> {
        self.baseline_value = None;
        if let Some(new_value) = self.tentative_value {
            self.value = new_value;
            self.tentative_value = None;
            Some(new_value)
        } else {
            None
        }
    }

    pub fn initialize(
        &mut self,
        screen_metrics: &shapes::ScreenMetrics,
        spritesheet: &mut sprites::SpriteSheet,
        shapes: &mut shapes::Shapes,
    ) {
        match &mut self.wt {
            WidgetClass::Knob(knob) => {
                knob.initialize(&self.rect, screen_metrics, self.value, spritesheet, shapes)
            }
            WidgetClass::VSlider(vslider) => {
                vslider.initialize(&self.rect, screen_metrics, self.value, spritesheet, shapes)
            }
            WidgetClass::Spinner(spinner) => {
                spinner.initialize(&self.rect, screen_metrics, spritesheet, shapes)
            }
            WidgetClass::Panel(panel) => {
                panel.initialize(&self.rect, screen_metrics, spritesheet, shapes)
            }
            WidgetClass::Toggle(toggle) => {
                toggle.initialize(&self.rect, screen_metrics, self.value, shapes)
            }
        };
    }

    pub fn update(
        &mut self,
        screen_metrics: &shapes::ScreenMetrics,
        spritesheet: &mut sprites::SpriteSheet,
        shapes: &mut shapes::Shapes,
        params: &Synchronizer,
    ) {
        let value = self.tentative_value.unwrap_or(self.value);

        let mut ctx = UpdateContext {
            meta: &self.meta,
            params,
            id: &self.id,
            rect: &self.rect,
            screen_metrics,
            spritesheet,
            shapes,
        };

        match &mut self.wt {
            WidgetClass::Knob(knob) => {
                knob.update(&mut ctx, value);
            }
            WidgetClass::VSlider(vslider) => {
                vslider.update(&mut ctx, value);
            }
            WidgetClass::Spinner(spinner) => {
                spinner.update(&mut ctx, value);
            }
            WidgetClass::Panel(_panel) => {}
            WidgetClass::Toggle(toggle) => {
                toggle.update(&mut ctx, value);
            }
        };
    }

    pub fn on_resize(
        &mut self,
        screen_metrics: &shapes::ScreenMetrics,
        spritesheet: &mut sprites::SpriteSheet,
        shapes: &mut shapes::Shapes,
        params: &Synchronizer,
    ) {
        let value = self.tentative_value.unwrap_or(self.value);
        let mut ctx = UpdateContext {
            meta: &self.meta,
            params,
            id: &self.id,
            rect: &self.rect,
            screen_metrics,
            spritesheet,
            shapes,
        };
        match &mut self.wt {
            WidgetClass::Knob(knob) => {
                knob.on_resize(&mut ctx, value);
            }
            WidgetClass::VSlider(vslider) => {
                vslider.on_resize(&mut ctx, value);
            }
            WidgetClass::Spinner(spinner) => {
                spinner.on_resize(&mut ctx, value);
            }
            WidgetClass::Panel(panel) => {
                panel.on_resize(&mut ctx, value);
            }
            WidgetClass::Toggle(toggle) => {
                toggle.on_resize(&mut ctx, value);
            }
        };
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Text {
    pub value: String,
    pub pos: LabelPosition,
    pub scale: f32,
    // TODO: color
}

#[derive(Debug)]
pub enum WidgetClass {
    Knob(Knob),
    VSlider(VSlider),
    Spinner(Spinner),
    Panel(Panel),
    Toggle(Toggle),
}

const KNOB_DEBUG_OUTLINE: bool = false;
const KNOB_DEBUG_OUTLINE_COLOR: [f32; 3] = [1.0, 0.0, 0.0];
const KNOB_OUTLINE_WIDTH: f32 = 0.001;
const KNOB_ARC_WIDTH: f32 = 0.001;

#[derive(Debug)]
pub struct Knob {
    polarity: Polarity,
    arc: shapes::Arc,
    arc_color: Color,
    notch_color: Color,
    _sprite_index: SpriteIndex,
    arc_index: ShapeIndex,
    inner_notch_index: ShapeIndex,
    outline_index: ShapeIndex,
    _circle_index: ShapeIndex,
    label: Option<Text>,
    value_text: Text,
    value_text_color: Color,
}

impl Knob {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rect: &Rect,
        polarity: Polarity,
        value: f64,
        arc_color: Color,
        notch_color: Color,
        label: Option<Text>,
        value_text: Text,
        value_text_color: Color,
    ) -> Self {
        let arc = Self::create_arc(rect, &polarity, value, &arc_color, KNOB_ARC_WIDTH);
        Knob {
            polarity,
            arc,
            arc_color,
            notch_color,
            _sprite_index: SpriteIndex(0),
            arc_index: ShapeIndex(0),
            inner_notch_index: ShapeIndex(0),
            outline_index: ShapeIndex(0),
            _circle_index: ShapeIndex(0),
            label,
            value_text,
            value_text_color,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_widget(
        meta: Arc<ParamsMeta>,
        id: WidgetId,
        rect: Rect,
        polarity: Polarity,
        value: f64,
        arc_color: Color,
        notch_color: Color,
        label: Option<Text>,
        value_text: Text,
        value_text_color: Color,
    ) -> Widget {
        let knob = Self::new(
            &rect,
            polarity,
            value,
            arc_color,
            notch_color,
            label,
            value_text,
            value_text_color,
        );
        Widget::new(meta, id, rect, value, WidgetClass::Knob(knob))
    }

    pub fn apply_to_texts<F: FnMut(&Text, &Color)>(&self, mut f: F) {
        if let Some(label) = &self.label {
            f(label, &DEFAULT_TEXT_COLOR);
        }
        f(&self.value_text, &self.value_text_color);
    }

    /// Utility function to generate an Arc for knobs.
    fn create_arc(
        rect: &Rect,
        polarity: &Polarity,
        value: f64,
        color: &Color,
        stroke_width: f32,
    ) -> shapes::Arc {
        let delta = rect.width().min(rect.height());
        let arc_radius = (delta / 2.0) * 0.85;

        let arc_x = rect.mid_x();
        let arc_y = rect.mid_y();

        shapes::Arc {
            x: arc_x,
            y: arc_y,
            radius: arc_radius,
            amount: value as f32,
            min_angle: 30.0,
            max_angle: -210.0,
            color: color.clone(),
            stroke_width, // : 0.008,
            polarity: polarity.clone(),
        }
    }

    fn create_notch(
        screen_metrics: &shapes::ScreenMetrics,
        arc: &shapes::Arc,
        value: f64,
        notch_color: &Color,
    ) -> shapes::Buffers {
        let from = (arc.x, arc.y);

        fn to_rad(angle: f32) -> f32 {
            angle * std::f32::consts::PI / 180.0
        }
        // TODO: Arc does 80%
        // TODO: Uh, fix this:
        let arc_range = arc.max_angle - arc.min_angle;
        let arc_value = value as f32 * arc_range;

        let angle = (arc_range - arc_value) + arc.min_angle;
        let delta_y = to_rad(angle).sin() * arc.radius;
        let delta_x = to_rad(angle).cos() * arc.radius;
        let to = (arc.x + delta_x, arc.y + delta_y);

        shapes::line_segment(&from, &to, screen_metrics, 0.003, &notch_color.to_array3())
    }

    pub fn scaled_to(value: f64, max: f64) -> f64 {
        (value * max).round()
    }

    #[inline(always)]
    pub fn delta_value(y: &f32, start_y: &f32, drag_factor: &f32) -> f32 {
        let delta = y - start_y;
        -delta * drag_factor
    }

    fn initialize(
        &mut self,
        rect: &Rect,
        screen_metrics: &shapes::ScreenMetrics,
        _value: f64,
        _spritesheet: &mut sprites::SpriteSheet,
        shapes: &mut shapes::Shapes,
    ) {
        // self.sprite_index = SpriteIndex(spritesheet.add(sprites::Sprite {
        //     pos: UserVec2::Rel(Vec2 {
        //         pos: [rect.pos[0], rect.pos[1]],
        //     }),
        //     size: UserVec2::Rel(Vec2 { pos: rect.size() }),
        //     src_px: sprites::SpriteSource {
        //         src_rect: [2.0, 1087.0, 250.0, 1328.0],
        //     },
        // }));
        let (vmargin, imargin) = (10, 10);
        let max_arc = Self::create_arc(rect, &self.polarity, 1.0, &self.arc_color, KNOB_ARC_WIDTH);
        let max_arc_buf = max_arc.render(screen_metrics);
        self.arc_index = ShapeIndex({
            let max_v_count = max_arc_buf.vertices.len() + vmargin;
            let max_i_count = max_arc_buf.indices.len() + imargin;

            shapes.add(shapes::Shape::from_lyon(
                self.arc.render(screen_metrics),
                max_v_count,
                max_i_count,
            ))
        });
        if KNOB_DEBUG_OUTLINE {
            self.outline_index = ShapeIndex({
                let buffers = shapes::rectangle_outline(
                    rect,
                    screen_metrics,
                    KNOB_OUTLINE_WIDTH,
                    &KNOB_DEBUG_OUTLINE_COLOR,
                );
                let max_v_count = buffers.vertices.len();
                let max_i_count = buffers.indices.len();
                shapes.add(shapes::Shape::from_lyon(buffers, max_v_count, max_i_count))
            });
        }
        self.inner_notch_index = ShapeIndex({
            let line_segment = Self::create_notch(screen_metrics, &max_arc, 1.0, &self.notch_color);
            let max_v_count = line_segment.vertices.len() + vmargin;
            let max_i_count = line_segment.indices.len() + imargin;
            shapes.add(shapes::Shape::from_lyon(
                line_segment,
                max_v_count,
                max_i_count,
            ))
        });

        // self.circle_index = ShapeIndex({
        //     let buffers = shapes::circle_outline(rect, screen_metrics, 0.001);
        //     let max_v_count = buffers.vertices.len();
        //     let max_i_count = buffers.indices.len();
        //     shapes.add(shapes::Shape::from_lyon(buffers, max_v_count, max_i_count))
        // });
    }

    fn on_resize(&mut self, ctx: &mut UpdateContext, value: f64) {
        self.update(ctx, value);
        if KNOB_DEBUG_OUTLINE {
            let buffers = shapes::rectangle_outline(
                ctx.rect,
                ctx.screen_metrics,
                KNOB_OUTLINE_WIDTH,
                &KNOB_DEBUG_OUTLINE_COLOR,
            );
            ctx.shapes
                .update(self.outline_index.0, &buffers.vertices, &buffers.indices);
        }

        // let buffers = shapes::circle_outline(ctx.rect, ctx.screen_metrics, 0.001);
        // ctx.shapes
        //     .update(self.circle_index.0, &buffers.vertices, &buffers.indices);
    }

    fn update(&mut self, ctx: &mut UpdateContext, value: f64) {
        // TODO: Add active_value()
        // let knob_scaled = Knob::scaled_to(value, 37.0); // for sprite index
        // let sprw = 244.0;
        // let (src_x1, src_y1) = (sprw * knob_scaled, 1087.0);
        // let (src_x2, src_y2) = (src_x1 + sprw, src_y1 + sprw);
        // spritesheet.update_sprite(
        //     self.sprite_index.0,
        //     &SpriteUpdate {
        //         pos: None,
        //         size: None,
        //         src_px: Some(sprites::SpriteSource {
        //             src_rect: [src_x1, src_y1, src_x2, src_y2],
        //         }),
        //     },
        // );

        self.arc.amount = value as f32;

        let arc_bufs = self.arc.render(ctx.screen_metrics);
        ctx.shapes
            .update(self.arc_index.0, &arc_bufs.vertices, &arc_bufs.indices);
        let line_segment =
            Self::create_notch(ctx.screen_metrics, &self.arc, value, &self.notch_color);
        ctx.shapes.update(
            self.inner_notch_index.0,
            &line_segment.vertices,
            &line_segment.indices,
        );

        // Update value label.
        if let WidgetId::Bound { eparam } = ctx.id {
            self.value_text.value = ctx.params.formatted_value(*eparam);
        }
    }

    fn on_dragging(
        &mut self,
        mouse_state: &ActiveMouseState,
        drag_factor: &f32,
        value: f64,
    ) -> f64 {
        let delta = Knob::delta_value(&mouse_state.pos.y, &mouse_state.start.y, drag_factor) as f64;
        (value + delta).min(1.0).max(0.0)
    }
}

const VSLIDER_OUTLINE_COLOR: [f32; 3] = [1.0, 1.0, 1.0];

#[derive(Debug)]
pub struct VSlider {
    outer_shape_index: ShapeIndex,
    thumb_index: ShapeIndex,
    thumb_size: Coord2,
}

impl VSlider {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        VSlider {
            outer_shape_index: ShapeIndex(0),
            thumb_index: ShapeIndex(0),
            thumb_size: Coord2::new(0.02, 0.02),
        }
    }

    pub fn new_widget(meta: Arc<ParamsMeta>, id: WidgetId, rect: Rect, value: f64) -> Widget {
        let vslider = Self::new();
        Widget::new(meta, id, rect, value, WidgetClass::VSlider(vslider))
    }

    fn initialize(
        &mut self,
        rect: &Rect,
        screen_metrics: &shapes::ScreenMetrics,
        value: f64,
        _spritesheet: &mut sprites::SpriteSheet,
        shapes: &mut shapes::Shapes,
    ) {
        let buffers =
            shapes::rectangle_outline(rect, screen_metrics, 0.001, &VSLIDER_OUTLINE_COLOR);
        let max_v_count = buffers.vertices.len();
        let max_i_count = buffers.indices.len();
        let outer_shape_index =
            shapes.add(shapes::Shape::from_lyon(buffers, max_v_count, max_i_count));

        // TODO: Value computation
        let buffers = Self::create_thumb(rect, screen_metrics, &self.thumb_size, value);
        let max_v_count = buffers.vertices.len();
        let max_i_count = buffers.indices.len();
        let thumb_index = shapes.add(shapes::Shape::from_lyon(buffers, max_v_count, max_i_count));

        self.outer_shape_index = ShapeIndex(outer_shape_index);
        self.thumb_index = ShapeIndex(thumb_index);
    }

    fn create_thumb(
        rect: &Rect,
        screen_metrics: &shapes::ScreenMetrics,
        thumb_size: &Coord2,
        value: f64,
    ) -> shapes::Buffers {
        let thumb_dist = rect.height() - thumb_size.y;
        let thumb_top = ((1.0 - value as f32) * thumb_dist) + rect.y1();
        let thumb_rect = Rect::new(rect.x1(), thumb_top, rect.x2(), thumb_top + thumb_size.y);
        shapes::rectangle_solid(&thumb_rect, screen_metrics)
    }

    #[inline(always)]
    pub fn delta_value(y: &f32, start_y: &f32, drag_factor: &f32) -> f32 {
        let delta = start_y - y;
        -delta * drag_factor
    }

    fn on_dragging(&mut self, rect: &Rect, mouse_state: &ActiveMouseState) -> f64 {
        self.updated_value_from(rect, mouse_state)
    }

    fn updated_value_from(&mut self, rect: &Rect, mouse_state: &ActiveMouseState) -> f64 {
        let dist = (rect.y2() - self.thumb_size.y / 2.0) - mouse_state.pos.y;
        let tot_dist = rect.y2() - rect.y1() - self.thumb_size.y;
        let target_value = dist / tot_dist;
        (target_value as f64).min(1.0).max(0.0)
    }

    fn update(&mut self, ctx: &mut UpdateContext, value: f64) {
        let buf = Self::create_thumb(ctx.rect, ctx.screen_metrics, &self.thumb_size, value);
        ctx.shapes
            .update(self.thumb_index.0, &buf.vertices, &buf.indices);
    }

    fn on_resize(&mut self, ctx: &mut UpdateContext, value: f64) {
        self.update(ctx, value);
        let buffers =
            shapes::rectangle_outline(ctx.rect, ctx.screen_metrics, 0.001, &VSLIDER_OUTLINE_COLOR);
        ctx.shapes.update(
            self.outer_shape_index.0,
            &buffers.vertices,
            &buffers.indices,
        );
    }
}

const SPINNER_OUTLINE: bool = false;
const SPINNER_OUTLINE_COLOR: [f32; 3] = [1.0, 1.0, 1.0];

#[derive(Debug)]
pub struct Spinner {
    outline_index: ShapeIndex,
    label: Option<Text>,
    value_text: Text,
    value_text_color: Color,
}

impl Spinner {
    pub fn new(label: Option<Text>, value_text: Text, value_text_color: Color) -> Self {
        Spinner {
            outline_index: ShapeIndex(0),
            label,
            value_text,
            value_text_color,
        }
    }

    pub fn new_widget(
        meta: Arc<ParamsMeta>,
        id: WidgetId,
        rect: Rect,
        value: f64,
        label: Option<Text>,
        value_text: Text,
        value_text_color: Color,
    ) -> Widget {
        let spinner = Self::new(label, value_text, value_text_color);
        Widget::new(meta, id, rect, value, WidgetClass::Spinner(spinner))
    }

    fn initialize(
        &mut self,
        rect: &Rect,
        screen_metrics: &shapes::ScreenMetrics,
        _spritesheet: &mut sprites::SpriteSheet,
        shapes: &mut shapes::Shapes,
    ) {
        //
        let buffers =
            shapes::rectangle_outline(rect, screen_metrics, 0.003, &SPINNER_OUTLINE_COLOR);
        let max_v_count = buffers.vertices.len();
        let max_i_count = buffers.indices.len();
        if SPINNER_OUTLINE {
            self.outline_index =
                ShapeIndex(shapes.add(shapes::Shape::from_lyon(buffers, max_v_count, max_i_count)));
        }
    }

    fn update(&mut self, ctx: &mut UpdateContext, _value: f64) {
        if let WidgetId::Bound { eparam } = ctx.id {
            self.value_text.value = ctx.params.formatted_value(*eparam);
        }
    }

    fn on_resize(&mut self, ctx: &mut UpdateContext, value: f64) {
        self.update(ctx, value);
        if SPINNER_OUTLINE {
            let buffers = shapes::rectangle_outline(
                ctx.rect,
                ctx.screen_metrics,
                0.003,
                &SPINNER_OUTLINE_COLOR,
            );
            ctx.shapes
                .update(self.outline_index.0, &buffers.vertices, &buffers.indices);
        }
    }

    fn on_dragging(
        &mut self,
        mouse_state: &ActiveMouseState,
        drag_factor: &f32,
        value: f64,
    ) -> f64 {
        let y = &mouse_state.pos.y;
        let start_y = &mouse_state.start.y;
        let delta = -(y - start_y) * drag_factor;
        let delta = delta as f64;
        (value + delta).min(1.0).max(0.0)
    }

    pub fn apply_to_texts<F: FnMut(&Text, &Color)>(&self, mut f: F) {
        if let Some(label) = &self.label {
            f(label, &DEFAULT_TEXT_COLOR);
        }
        f(&self.value_text, &self.value_text_color);
    }
}

const PANEL_OUTLINE_COLOR: [f32; 3] = [1.0, 1.0, 1.0];

#[derive(Debug)]
pub struct Panel {
    outline_index: ShapeIndex,
    label: Option<Text>,
}

impl Panel {
    pub fn new(label: Option<Text>) -> Self {
        Panel {
            outline_index: ShapeIndex(0),
            label,
        }
    }

    pub fn new_widget(
        meta: Arc<ParamsMeta>,
        id: WidgetId,
        rect: Rect,
        label: Option<Text>,
    ) -> Widget {
        let panel = Self::new(label);
        Widget::new(meta, id, rect, 0.0, WidgetClass::Panel(panel))
    }

    fn initialize(
        &mut self,
        rect: &Rect,
        screen_metrics: &shapes::ScreenMetrics,
        _spritesheet: &mut sprites::SpriteSheet,
        shapes: &mut shapes::Shapes,
    ) {
        let buffers = shapes::rectangle_outline(rect, screen_metrics, 0.003, &PANEL_OUTLINE_COLOR);
        let max_v_count = buffers.vertices.len();
        let max_i_count = buffers.indices.len();
        self.outline_index =
            ShapeIndex(shapes.add(shapes::Shape::from_lyon(buffers, max_v_count, max_i_count)));
    }

    fn on_resize(&mut self, ctx: &mut UpdateContext, _value: f64) {
        let buffers =
            shapes::rectangle_outline(ctx.rect, ctx.screen_metrics, 0.003, &PANEL_OUTLINE_COLOR);
        ctx.shapes
            .update(self.outline_index.0, &buffers.vertices, &buffers.indices);
    }

    pub fn apply_to_texts<F: FnMut(&Text, &Color)>(&self, mut f: F) {
        if let Some(label) = &self.label {
            f(label, &DEFAULT_TEXT_COLOR);
        }
    }
}

#[derive(Debug)]
pub struct Toggle {
    _outline_index: ShapeIndex,
    thumb_index: ShapeIndex,
    label: Option<Text>,
}

impl Toggle {
    pub fn new(label: Option<Text>) -> Self {
        Toggle {
            _outline_index: ShapeIndex(0),
            thumb_index: ShapeIndex(0),
            label,
        }
    }

    pub fn new_widget(
        meta: Arc<ParamsMeta>,
        id: WidgetId,
        rect: Rect,
        value: f64,
        label: Option<Text>,
    ) -> Widget {
        let toggle = Self::new(label);
        Widget::new(meta, id, rect, value, WidgetClass::Toggle(toggle))
    }

    fn initialize(
        &mut self,
        rect: &Rect,
        screen_metrics: &shapes::ScreenMetrics,
        value: f64,
        shapes: &mut shapes::Shapes,
    ) {
        // let buffers = Self::create_outline(rect, screen_metrics);
        // let max_v_count = buffers.vertices.len();
        // let max_i_count = buffers.indices.len();
        // let outline_index = shapes.add(shapes::Shape::from_lyon(buffers, max_v_count, max_i_count));

        // TODO: Value computation
        let buffers = Self::create_thumb(rect, screen_metrics, value);
        let max_v_count = buffers.vertices.len();
        let max_i_count = buffers.indices.len();
        let thumb_index = shapes.add(shapes::Shape::from_lyon(buffers, max_v_count, max_i_count));

        //self.outline_index = ShapeIndex(outline_index);
        self.thumb_index = ShapeIndex(thumb_index);
    }

    fn create_thumb(
        rect: &Rect,
        screen_metrics: &shapes::ScreenMetrics,
        value: f64,
    ) -> shapes::Buffers {
        let contracted = rect.contract(0.01);
        let r = if value >= 0.5 { rect } else { &contracted };
        shapes::rectangle_solid(r, screen_metrics)
    }

    fn _create_outline(rect: &Rect, screen_metrics: &shapes::ScreenMetrics) -> shapes::Buffers {
        shapes::rectangle_outline(rect, screen_metrics, 0.001, &TOGGLE_OUTLINE_COLOR)
    }

    fn on_dragging(&mut self, value: f64) -> f64 {
        if value >= 0.5 {
            0.0
        } else {
            1.0
        }
    }

    fn update(&mut self, ctx: &mut UpdateContext, value: f64) {
        //let buffers = Self::create_outline(ctx.rect, ctx.screen_metrics);
        //ctx.shapes.update(self.outline_index.0, &buffers.vertices, &buffers.indices);
        let buffers = Self::create_thumb(ctx.rect, ctx.screen_metrics, value);
        ctx.shapes
            .update(self.thumb_index.0, &buffers.vertices, &buffers.indices);
    }

    fn on_resize(&mut self, ctx: &mut UpdateContext, value: f64) {
        // let buffers = Self::create_outline(ctx.rect, ctx.screen_metrics);
        // ctx.shapes
        //     .update(self.outline_index.0, &buffers.vertices, &buffers.indices);
        let buffers = Self::create_thumb(ctx.rect, ctx.screen_metrics, value);
        ctx.shapes
            .update(self.thumb_index.0, &buffers.vertices, &buffers.indices);
    }

    pub fn apply_to_texts<F: FnMut(&Text, &Color)>(&self, mut f: F) {
        if let Some(label) = &self.label {
            f(label, &DEFAULT_TEXT_COLOR);
        }
    }
}
