pub mod knob;
pub mod panel;
pub mod spinner;
pub mod toggle;
pub mod vslider;

use std::sync::Arc;

use serde::Deserialize;

use crate::params::sync::Synchronizer;
use crate::params::{EParam, ParamsMeta};

use crate::ui::alignment::{HorizontalAlign, VerticalAlign};
use crate::ui::coords::Rect;
use crate::ui::shapes;
use crate::ui::shapes::{Color, ScreenMetrics};
use crate::ui::sprites;
use crate::ui::widgets::{
    knob::Knob, panel::Panel, spinner::Spinner, toggle::Toggle, vslider::VSlider,
};
use crate::ui::window::ActiveMouseState;

pub const DEFAULT_TEXT_COLOR: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
};

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

pub struct UpdateContext<'a> {
    #[allow(dead_code)]
    pub meta: &'a Arc<ParamsMeta>,
    pub params: &'a Synchronizer,
    pub id: &'a WidgetId,
    pub rect: &'a Rect,
    pub screen_metrics: &'a ScreenMetrics,
    #[allow(dead_code)]
    pub spritesheet: &'a mut sprites::SpriteSheet,
    pub shapes: &'a mut shapes::Shapes,
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
            WidgetClass::VSlider(vslider) => vslider.apply_to_texts(f),
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
        screen_metrics: &ScreenMetrics,
        spritesheet_builder: &mut sprites::SpriteSheetBuilder,
        shapes_builder: &mut shapes::ShapesBuilder,
    ) {
        match &mut self.wt {
            WidgetClass::Knob(knob) => knob.initialize(
                &self.rect,
                screen_metrics,
                self.value,
                spritesheet_builder,
                shapes_builder,
            ),
            WidgetClass::VSlider(vslider) => vslider.initialize(
                &self.rect,
                screen_metrics,
                self.value,
                spritesheet_builder,
                shapes_builder,
            ),
            WidgetClass::Spinner(spinner) => spinner.initialize(
                &self.rect,
                screen_metrics,
                spritesheet_builder,
                shapes_builder,
            ),
            WidgetClass::Panel(panel) => panel.initialize(
                &self.rect,
                screen_metrics,
                spritesheet_builder,
                shapes_builder,
            ),
            WidgetClass::Toggle(toggle) => toggle.initialize(
                &self.rect,
                screen_metrics,
                self.value,
                spritesheet_builder,
                shapes_builder,
            ),
        };
    }

    pub fn update(
        &mut self,
        screen_metrics: &ScreenMetrics,
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
        screen_metrics: &ScreenMetrics,
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
