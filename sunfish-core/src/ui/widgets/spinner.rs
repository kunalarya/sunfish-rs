use std::sync::Arc;

use crate::params::ParamsMeta;
use crate::ui::buffer_memory::GpuShape;
use crate::ui::coords::Rect;
use crate::ui::shape_util;
use crate::ui::shapes;
use crate::ui::shapes::{Color, ScreenMetrics};
use crate::ui::sprites;
use crate::ui::window::ActiveMouseState;

use crate::ui::widgets::{self, ShapeIndex, Text, UpdateContext, Widget, WidgetClass, WidgetId};

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

    pub fn initialize(
        &mut self,
        rect: &Rect,
        screen_metrics: &ScreenMetrics,
        _spritesheet_builder: &mut sprites::SpriteSheetBuilder,
        shapes_builder: &mut shapes::ShapesBuilder,
    ) {
        //
        let buffers =
            shape_util::rectangle_outline(rect, screen_metrics, 0.003, &SPINNER_OUTLINE_COLOR);
        let max_v_count = buffers.vertices.len();
        let max_i_count = buffers.indices.len();
        if SPINNER_OUTLINE {
            self.outline_index = ShapeIndex(shapes_builder.add(GpuShape::from_lyon(
                buffers,
                max_v_count,
                max_i_count,
            )));
        }
    }

    pub fn update(&mut self, ctx: &mut UpdateContext, _value: f64) {
        if let WidgetId::Bound { eparam } = ctx.id {
            self.value_text.value = ctx.params.formatted_value(*eparam);
        }
    }

    pub fn on_resize(&mut self, ctx: &mut UpdateContext, value: f64) {
        self.update(ctx, value);
        if SPINNER_OUTLINE {
            let buffers = shape_util::rectangle_outline(
                ctx.rect,
                ctx.screen_metrics,
                0.003,
                &SPINNER_OUTLINE_COLOR,
            );
            ctx.shapes
                .update(self.outline_index.0, &buffers.vertices, &buffers.indices);
        }
    }

    pub fn on_dragging(
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
            f(label, &widgets::DEFAULT_TEXT_COLOR);
        }
        f(&self.value_text, &self.value_text_color);
    }
}
