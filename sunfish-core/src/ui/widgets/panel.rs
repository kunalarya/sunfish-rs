use std::sync::Arc;

use crate::params::ParamsMeta;
use crate::ui::buffer_memory::GpuShape;
use crate::ui::coords::Rect;
use crate::ui::shape_util;
use crate::ui::shapes;
use crate::ui::shapes::{Color, ScreenMetrics};
use crate::ui::sprites;

use crate::ui::widgets::{self, ShapeIndex, Text, UpdateContext, Widget, WidgetClass, WidgetId};

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

    pub fn initialize(
        &mut self,
        rect: &Rect,
        screen_metrics: &ScreenMetrics,
        _spritesheet_builder: &mut sprites::SpriteSheetBuilder,
        shapes_builder: &mut shapes::ShapesBuilder,
    ) {
        let buffers =
            shape_util::rectangle_outline(rect, screen_metrics, 0.003, &PANEL_OUTLINE_COLOR);
        let max_v_count = buffers.vertices.len();
        let max_i_count = buffers.indices.len();
        self.outline_index =
            ShapeIndex(shapes_builder.add(GpuShape::from_lyon(buffers, max_v_count, max_i_count)));
    }

    pub fn on_resize(&mut self, ctx: &mut UpdateContext, _value: f64) {
        let buffers = shape_util::rectangle_outline(
            ctx.rect,
            ctx.screen_metrics,
            0.003,
            &PANEL_OUTLINE_COLOR,
        );
        ctx.shapes
            .update(self.outline_index.0, &buffers.vertices, &buffers.indices);
    }

    pub fn apply_to_texts<F: FnMut(&Text, &Color)>(&self, mut f: F) {
        if let Some(label) = &self.label {
            f(label, &widgets::DEFAULT_TEXT_COLOR);
        }
    }
}
