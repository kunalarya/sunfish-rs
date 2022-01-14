use std::sync::Arc;

use serde::Deserialize;

use crate::params::ParamsMeta;
use crate::ui::buffer_memory::GpuShape;
use crate::ui::coords::{Coord2, Rect, UserVec2, Vec2};
use crate::ui::shape_util;
use crate::ui::shapes;
use crate::ui::shapes::{Color, ScreenMetrics};
use crate::ui::sprites;
use crate::ui::window::ActiveMouseState;

use crate::ui::widgets::{
    ShapeIndex, SpriteIndex, Text, UpdateContext, Widget, WidgetClass, WidgetId,
};

const VSLIDER_DEBUG_OUTLINE: bool = false;
const VSLIDER_DEBUG_OUTLINE_COLOR: [f32; 3] = [1.0, 0.0, 0.0];

#[derive(Debug)]
pub struct VSlider {
    outer_shape_index: ShapeIndex,
    thumb_index: ShapeIndex,
    thumb_size: Coord2,
    sprite_info: Option<VSliderSprite>,
    thumb_sprite_index: Option<SpriteIndex>,
    value_text: Text,
    value_text_color: Color,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VSliderSprite {
    active: Rect,
}

impl VSlider {
    #[allow(clippy::new_without_default)]
    pub fn new(
        sprite_info: Option<VSliderSprite>,
        value_text: Text,
        value_text_color: Color,
    ) -> Self {
        VSlider {
            outer_shape_index: ShapeIndex(0),
            thumb_index: ShapeIndex(0),
            thumb_size: Coord2::new(0.02, 0.02),
            sprite_info,
            thumb_sprite_index: None,
            value_text,
            value_text_color,
        }
    }

    pub fn new_widget(
        meta: Arc<ParamsMeta>,
        id: WidgetId,
        rect: Rect,
        value: f64,
        sprite_info: Option<VSliderSprite>,
        value_text: Text,
        value_text_color: Color,
    ) -> Widget {
        let vslider = Self::new(sprite_info, value_text, value_text_color);
        Widget::new(meta, id, rect, value, WidgetClass::VSlider(vslider))
    }

    pub fn initialize(
        &mut self,
        rect: &Rect,
        screen_metrics: &ScreenMetrics,
        value: f64,
        spritesheet_builder: &mut sprites::SpriteSheetBuilder,
        shapes_builder: &mut shapes::ShapesBuilder,
    ) {
        if VSLIDER_DEBUG_OUTLINE {
            let buffers = shape_util::rectangle_outline(
                rect,
                screen_metrics,
                0.001,
                &VSLIDER_DEBUG_OUTLINE_COLOR,
            );
            let max_v_count = buffers.vertices.len();
            let max_i_count = buffers.indices.len();
            let outer_shape_index =
                shapes_builder.add(GpuShape::from_lyon(buffers, max_v_count, max_i_count));
            self.outer_shape_index = ShapeIndex(outer_shape_index);
        }
        let thumb_rect = Self::thumb_rect(rect, &self.thumb_size, value);
        if let Some(sprite_info) = &self.sprite_info {
            self.thumb_sprite_index = Some(SpriteIndex(spritesheet_builder.add(
                sprites::SpriteBuilder {
                    pos: UserVec2::Rel(Vec2 {
                        pos: [thumb_rect.pos[0], thumb_rect.pos[1]],
                    }),
                    size: UserVec2::Rel(Vec2 {
                        pos: thumb_rect.size(),
                    }),
                    src_px: sprites::SpriteSource {
                        src_rect: sprite_info.active.pos,
                    },
                },
            )));
        } else {
            // TODO: Value computation
            let buffers = shape_util::rectangle_solid(&thumb_rect, screen_metrics);
            let max_v_count = buffers.vertices.len();
            let max_i_count = buffers.indices.len();
            let thumb_index =
                shapes_builder.add(GpuShape::from_lyon(buffers, max_v_count, max_i_count));

            self.thumb_index = ShapeIndex(thumb_index);
        }
    }

    fn thumb_rect(rect: &Rect, thumb_size: &Coord2, value: f64) -> Rect {
        let thumb_dist = rect.height() - thumb_size.y;
        let thumb_top = ((1.0 - value as f32) * thumb_dist) + rect.y1();
        Rect::new(rect.x1(), thumb_top, rect.x2(), thumb_top + thumb_size.y)
    }

    #[inline(always)]
    pub fn delta_value(y: &f32, start_y: &f32, drag_factor: &f32) -> f32 {
        let delta = start_y - y;
        -delta * drag_factor
    }

    pub fn on_dragging(&mut self, rect: &Rect, mouse_state: &ActiveMouseState) -> f64 {
        self.updated_value_from(rect, mouse_state)
    }

    fn updated_value_from(&mut self, rect: &Rect, mouse_state: &ActiveMouseState) -> f64 {
        let dist = (rect.y2() - self.thumb_size.y / 2.0) - mouse_state.pos.y;
        let tot_dist = rect.y2() - rect.y1() - self.thumb_size.y;
        let target_value = dist / tot_dist;
        (target_value as f64).min(1.0).max(0.0)
    }

    pub fn update(&mut self, ctx: &mut UpdateContext, value: f64) {
        let thumb_rect = Self::thumb_rect(ctx.rect, &self.thumb_size, value);
        if self.sprite_info.is_some() {
            ctx.spritesheet.update_sprite(
                self.thumb_sprite_index.unwrap().0,
                &sprites::SpriteUpdate {
                    pos: Some(UserVec2::Rel(Vec2 {
                        pos: [thumb_rect.pos[0], thumb_rect.pos[1]],
                    })),
                    ..Default::default()
                },
                ctx.screen_metrics,
            );
        } else {
            let buf = shape_util::rectangle_solid(&thumb_rect, ctx.screen_metrics);
            ctx.shapes
                .update(self.thumb_index.0, &buf.vertices, &buf.indices);
        }

        // Update value label.
        if let WidgetId::Bound { eparam } = ctx.id {
            self.value_text.value = ctx.params.formatted_value(*eparam);
        }
    }

    pub fn on_resize(&mut self, ctx: &mut UpdateContext, value: f64) {
        self.update(ctx, value);
        if VSLIDER_DEBUG_OUTLINE {
            let buffers = shape_util::rectangle_outline(
                ctx.rect,
                ctx.screen_metrics,
                0.001,
                &VSLIDER_DEBUG_OUTLINE_COLOR,
            );
            ctx.shapes.update(
                self.outer_shape_index.0,
                &buffers.vertices,
                &buffers.indices,
            );
        }
    }

    pub fn apply_to_texts<F: FnMut(&Text, &Color)>(&self, mut f: F) {
        f(&self.value_text, &self.value_text_color);
    }
}
