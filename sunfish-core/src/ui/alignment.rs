use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

impl HorizontalAlign {
    pub fn to_wgpu(&self) -> wgpu_glyph::HorizontalAlign {
        match self {
            Self::Left => wgpu_glyph::HorizontalAlign::Left,
            Self::Center => wgpu_glyph::HorizontalAlign::Center,
            Self::Right => wgpu_glyph::HorizontalAlign::Right,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerticalAlign {
    Top,
    Center,
    Bottom,
}

impl VerticalAlign {
    pub fn to_wgpu(&self) -> wgpu_glyph::VerticalAlign {
        match self {
            Self::Top => wgpu_glyph::VerticalAlign::Top,
            Self::Center => wgpu_glyph::VerticalAlign::Center,
            Self::Bottom => wgpu_glyph::VerticalAlign::Bottom,
        }
    }
}
