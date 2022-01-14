use std::sync::Arc;

use serde::Deserialize;

use crate::params::ParamsMeta;
use crate::ui::buffer_memory::GpuShape;
use crate::ui::coords::{Rect, UserVec2, Vec2};
use crate::ui::shape_util;
use crate::ui::shapes;
use crate::ui::shapes::{Color, ScreenMetrics};
use crate::ui::sprites;

use crate::ui::widgets::{
    self, ShapeIndex, SpriteIndex, Text, UpdateContext, Widget, WidgetClass, WidgetId,
};

#[allow(dead_code)]
const TOGGLE_OUTLINE_COLOR: [f32; 3] = [1.0, 0.0, 0.0];

#[derive(Debug)]
pub struct Toggle {
    _outline_index: ShapeIndex,
    thumb_index: ShapeIndex,
    label: Option<Text>,
    sprite_info: Option<ToggleSprite>,
    sprite_index: Option<SpriteIndex>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ToggleSprite {
    on: Rect,
    off: Rect,
}

impl Toggle {
    pub fn new(label: Option<Text>, sprite_info: Option<ToggleSprite>) -> Self {
        Toggle {
            _outline_index: ShapeIndex(0),
            thumb_index: ShapeIndex(0),
            label,
            sprite_info,
            sprite_index: None,
        }
    }

    pub fn new_widget(
        meta: Arc<ParamsMeta>,
        id: WidgetId,
        rect: Rect,
        value: f64,
        label: Option<Text>,
        sprite_info: Option<ToggleSprite>,
    ) -> Widget {
        let toggle = Self::new(label, sprite_info);
        Widget::new(meta, id, rect, value, WidgetClass::Toggle(toggle))
    }

    pub fn initialize(
        &mut self,
        rect: &Rect,
        screen_metrics: &ScreenMetrics,
        value: f64,
        spritesheet_builder: &mut sprites::SpriteSheetBuilder,
        shapes_builder: &mut shapes::ShapesBuilder,
    ) {
        // TODO: Value computation
        let value = Self::value_to_bool(value);

        if let Some(sprite_info) = &self.sprite_info {
            self.sprite_index = Some(SpriteIndex(spritesheet_builder.add(
                sprites::SpriteBuilder {
                    pos: UserVec2::Rel(Vec2 {
                        pos: [rect.pos[0], rect.pos[1]],
                    }),
                    size: UserVec2::Rel(Vec2 { pos: rect.size() }),
                    src_px: sprites::SpriteSource {
                        src_rect: sprite_info.on.pos,
                    },
                },
            )));
        } else {
            let buffers = Self::create_toggle(rect, screen_metrics, value);
            let max_v_count = buffers.vertices.len();
            let max_i_count = buffers.indices.len();
            let thumb_index =
                shapes_builder.add(GpuShape::from_lyon(buffers, max_v_count, max_i_count));
            self.thumb_index = ShapeIndex(thumb_index);
        }
    }

    fn create_toggle(rect: &Rect, screen_metrics: &ScreenMetrics, value: bool) -> shapes::Buffers {
        let contracted = rect.contract(0.01);
        let r = if value { rect } else { &contracted };
        shape_util::rectangle_solid(r, screen_metrics)
    }

    fn value_to_bool(value: f64) -> bool {
        value >= 0.5
    }

    fn _create_outline(rect: &Rect, screen_metrics: &ScreenMetrics) -> shapes::Buffers {
        shape_util::rectangle_outline(rect, screen_metrics, 0.001, &TOGGLE_OUTLINE_COLOR)
    }

    pub fn on_dragging(&mut self, value: f64) -> f64 {
        if value >= 0.5 {
            0.0
        } else {
            1.0
        }
    }

    pub fn update(&mut self, ctx: &mut UpdateContext, value: f64) {
        let value = Self::value_to_bool(value);
        //println!("Toggle::update({:?})", self.sprite_info);
        if let Some(sprite_info) = &self.sprite_info {
            ctx.spritesheet.update_sprite(
                self.sprite_index.unwrap().0,
                &sprites::SpriteUpdate {
                    src_px: Some(sprites::SpriteSource {
                        src_rect: if value {
                            sprite_info.on.pos
                        } else {
                            sprite_info.off.pos
                        },
                    }),
                    ..Default::default()
                },
                ctx.screen_metrics,
            );
        } else {
            let buffers = Self::create_toggle(ctx.rect, ctx.screen_metrics, value);
            ctx.shapes
                .update(self.thumb_index.0, &buffers.vertices, &buffers.indices);
        }
    }

    pub fn on_resize(&mut self, ctx: &mut UpdateContext, value: f64) {
        if let Some(_sprite_info) = &self.sprite_info {
            // TODO?
        } else {
            let value = Self::value_to_bool(value);
            let buffers = Self::create_toggle(ctx.rect, ctx.screen_metrics, value);
            ctx.shapes
                .update(self.thumb_index.0, &buffers.vertices, &buffers.indices);
        }
    }

    pub fn apply_to_texts<F: FnMut(&Text, &Color)>(&self, mut f: F) {
        if let Some(label) = &self.label {
            f(label, &widgets::DEFAULT_TEXT_COLOR);
        }
    }
}
