use std::sync::Arc;

use ron::de::from_str;
use serde::Deserialize;

use crate::params::ParamsMeta;
use crate::ui::coords::Rect;
use crate::ui::shapes::{Color, Polarity};
use crate::ui::widgets;
use crate::ui::widgets::{knob, panel, spinner, toggle, vslider};

#[derive(Clone, Debug, Deserialize)]
pub struct Styling {
    pub size: (i32, i32),
    pub background: Background,
    pub padding: (f32, f32),
    pub stylesheet_image: Option<String>,
    elements: Vec<Element>,
}

#[derive(Clone, Debug, Deserialize)]
pub enum Background {
    Solid { color: Color },
    Sprite { dest_rect: Rect, src_rect: Rect },
}

#[derive(Clone, Debug, Deserialize)]
pub enum Element {
    Knob {
        widget_id: widgets::WidgetId,
        rect: Rect,
        arc_color: Color,
        notch_color: Color,
        label: Option<widgets::Text>,
        value_text: widgets::Text,
        value_text_color: Color,
        polarity: Option<Polarity>,
    },
    VSlider {
        widget_id: widgets::WidgetId,
        rect: Rect,
        sprite: Option<vslider::VSliderSprite>,
        value_text: widgets::Text,
        value_text_color: Color,
    },
    Spinner {
        widget_id: widgets::WidgetId,
        rect: Rect,
        label: Option<widgets::Text>,
        value_text: widgets::Text,
        value_text_color: Color,
    },
    Toggle {
        widget_id: widgets::WidgetId,
        rect: Rect,
        label: Option<widgets::Text>,
        sprite: Option<toggle::ToggleSprite>,
    },
    Panel {
        rect: Rect,
        color: Color,
        label: Option<widgets::Text>,
    },
}

pub fn load_default() -> Styling {
    let styling_filename = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("styling.ron");
    load_widgets_from_file(styling_filename.to_str().unwrap())
}

pub fn load_widgets_from_file(filename: &str) -> Styling {
    let definitions = std::fs::read(filename).unwrap();
    let definitions = std::str::from_utf8(&definitions).unwrap();
    let styling: Styling = match from_str(definitions) {
        Ok(x) => x,
        Err(e) => {
            panic!("Failed to load config: {}", e);
        }
    };
    println!(
        "GUI styles width={}, height={}",
        styling.size.0, styling.size.1
    );
    styling
}

pub fn create_widgets(def: &Styling, meta: Arc<ParamsMeta>) -> Vec<widgets::Widget> {
    let mut widgets = vec![];
    let mut uniq_id = 0;

    for elm in &def.elements {
        match elm {
            Element::Knob {
                widget_id,
                rect,
                arc_color,
                notch_color,
                label,
                value_text,
                value_text_color,
                polarity,
            } => {
                widgets.push(knob::Knob::new_widget(
                    Arc::clone(&meta),
                    *widget_id,
                    rect.clone(),
                    polarity.clone().unwrap_or(Polarity::Unipolar),
                    0.0,
                    arc_color.clone(),
                    notch_color.clone(),
                    label.clone(),
                    value_text.clone(),
                    value_text_color.clone(),
                ));
            }
            Element::Panel {
                rect,
                label,
                // TODO: color
                ..
            } => {
                uniq_id += 1;
                widgets.push(panel::Panel::new_widget(
                    Arc::clone(&meta),
                    widgets::WidgetId::Unspecified { id: uniq_id },
                    rect.clone(),
                    label.clone(),
                ));
            }
            Element::Spinner {
                widget_id,
                rect,
                label,
                value_text,
                value_text_color,
            } => {
                widgets.push(spinner::Spinner::new_widget(
                    Arc::clone(&meta),
                    *widget_id,
                    rect.clone(),
                    0.0,
                    label.clone(),
                    value_text.clone(),
                    value_text_color.clone(),
                ));
            }
            Element::Toggle {
                widget_id,
                rect,
                label,
                sprite,
            } => {
                widgets.push(toggle::Toggle::new_widget(
                    Arc::clone(&meta),
                    *widget_id,
                    rect.clone(),
                    0.0,
                    label.clone(),
                    sprite.clone(),
                ));
            }
            Element::VSlider {
                widget_id,
                rect,
                sprite,
                value_text,
                value_text_color,
            } => {
                widgets.push(vslider::VSlider::new_widget(
                    Arc::clone(&meta),
                    *widget_id,
                    rect.clone(),
                    0.0,
                    sprite.clone(),
                    value_text.clone(),
                    value_text_color.clone(),
                ));
            }
        }
    }

    widgets
}
