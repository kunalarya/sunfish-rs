use std::sync::Arc;

use crate::params::ParamsMeta;
use crate::ui::buffer_memory::GpuShape;
use crate::ui::coords::Rect;
use crate::ui::shape_util;
use crate::ui::shapes;
use crate::ui::shapes::{Color, Polarity, ScreenMetrics};
use crate::ui::sprites;
use crate::ui::window::ActiveMouseState;

use crate::ui::widgets::{
    self, ShapeIndex, SpriteIndex, Text, UpdateContext, Widget, WidgetClass, WidgetId,
};

const KNOB_DEBUG_OUTLINE: bool = false;
const KNOB_DEBUG_OUTLINE_COLOR: [f32; 3] = [1.0, 0.0, 0.0];

const KNOB_OUTLINE_WIDTH: f32 = 0.001;
const KNOB_ARC_WIDTH: f32 = 0.001;

#[derive(Debug)]
pub struct Knob {
    polarity: Polarity,
    arc: shape_util::Arc,
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
            f(label, &widgets::DEFAULT_TEXT_COLOR);
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
    ) -> shape_util::Arc {
        let delta = rect.width().min(rect.height());
        let arc_radius = (delta / 2.0) * 0.85;

        let arc_x = rect.mid_x();
        let arc_y = rect.mid_y();

        shape_util::Arc {
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
        screen_metrics: &ScreenMetrics,
        arc: &shape_util::Arc,
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

        shape_util::line_segment(&from, &to, screen_metrics, 0.003, &notch_color.to_array3())
    }

    pub fn scaled_to(value: f64, max: f64) -> f64 {
        (value * max).round()
    }

    #[inline(always)]
    pub fn delta_value(y: &f32, start_y: &f32, drag_factor: &f32) -> f32 {
        let delta = y - start_y;
        -delta * drag_factor
    }

    pub fn initialize(
        &mut self,
        rect: &Rect,
        screen_metrics: &ScreenMetrics,
        _value: f64,
        _spritesheet_builder: &mut sprites::SpriteSheetBuilder,
        shapes_builder: &mut shapes::ShapesBuilder,
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

            shapes_builder.add(GpuShape::from_lyon(
                self.arc.render(screen_metrics),
                max_v_count,
                max_i_count,
            ))
        });
        if KNOB_DEBUG_OUTLINE {
            self.outline_index = ShapeIndex({
                let buffers = shape_util::rectangle_outline(
                    rect,
                    screen_metrics,
                    KNOB_OUTLINE_WIDTH,
                    &KNOB_DEBUG_OUTLINE_COLOR,
                );
                let max_v_count = buffers.vertices.len();
                let max_i_count = buffers.indices.len();
                shapes_builder.add(GpuShape::from_lyon(buffers, max_v_count, max_i_count))
            });
        }
        self.inner_notch_index = ShapeIndex({
            let line_segment = Self::create_notch(screen_metrics, &max_arc, 1.0, &self.notch_color);
            let max_v_count = line_segment.vertices.len() + vmargin;
            let max_i_count = line_segment.indices.len() + imargin;
            shapes_builder.add(GpuShape::from_lyon(line_segment, max_v_count, max_i_count))
        });

        // self.circle_index = ShapeIndex({
        //     let buffers = shape_util::circle_outline(rect, screen_metrics, 0.001);
        //     let max_v_count = buffers.vertices.len();
        //     let max_i_count = buffers.indices.len();
        //     shapes.add(Shape::from_lyon(buffers, max_v_count, max_i_count))
        // });
    }

    pub fn on_resize(&mut self, ctx: &mut UpdateContext, value: f64) {
        self.update(ctx, value);
        if KNOB_DEBUG_OUTLINE {
            let buffers = shape_util::rectangle_outline(
                ctx.rect,
                ctx.screen_metrics,
                KNOB_OUTLINE_WIDTH,
                &KNOB_DEBUG_OUTLINE_COLOR,
            );
            ctx.shapes
                .update(self.outline_index.0, &buffers.vertices, &buffers.indices);
        }

        // let buffers = shape_util::circle_outline(ctx.rect, ctx.screen_metrics, 0.001);
        // ctx.shapes
        //     .update(self.circle_index.0, &buffers.vertices, &buffers.indices);
    }

    pub fn update(&mut self, ctx: &mut UpdateContext, value: f64) {
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

    pub fn on_dragging(
        &mut self,
        mouse_state: &ActiveMouseState,
        drag_factor: &f32,
        value: f64,
    ) -> f64 {
        let delta = Knob::delta_value(&mouse_state.pos.y, &mouse_state.start.y, drag_factor) as f64;
        (value + delta).min(1.0).max(0.0)
    }
}
