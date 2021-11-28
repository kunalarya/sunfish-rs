use std::mem;

use bytemuck::{Pod, Zeroable};
use euclid::vec2;
use lyon::geom::euclid;
use lyon::geom::euclid::Box2D;
use lyon::math::{point, Point};
use lyon::math::{Angle, Vector};
use lyon::path::builder::Build;
use lyon::path::Path;
use lyon::path::Polygon;
use lyon::tessellation;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, LineJoin, StrokeOptions, StrokeTessellator,
};
use lyon::tessellation::{FillVertex, StrokeVertex};
use serde::Deserialize;

use crate::ui::coords::Rect;
use crate::ui::packed_shapes;

pub type Buffers = tessellation::VertexBuffers<ShapeVertex, u16>;

#[derive(Clone, Debug, Deserialize)]
pub enum Polarity {
    Unipolar,
    Bipolar,
}

// TODO: move to Coords
pub struct ScreenMetrics {
    pub width_f32: f32,
    pub height_f32: f32,
    pub width_u32: u32,
    pub height_u32: u32,
    pub scale_factor: f64,
    pub ratio: f32,
}

impl ScreenMetrics {
    pub fn new(width: u32, height: u32, scale_factor: f64) -> Self {
        let width_f32 = width as f32;
        let height_f32 = height as f32;
        let ratio = width_f32 / height_f32;
        log::info!(
            "width={} height={} width_f32={} height_f32={} ratio={:?}",
            width,
            height,
            width_f32,
            height_f32,
            ratio
        );
        ScreenMetrics {
            width_f32,
            height_f32,
            width_u32: width,
            height_u32: height,
            scale_factor,
            ratio,
        }
    }

    pub fn norm_x_to_corrected(&self, x: f32) -> f32 {
        x
    }

    pub fn norm_y_to_corrected(&self, y: f32) -> f32 {
        y * self.ratio
    }

    pub fn corrected_x_to_norm(&self, x: f32) -> f32 {
        x
    }

    pub fn corrected_y_to_norm(&self, y: f32) -> f32 {
        y / self.ratio
    }

    pub fn screen_x_to_norm(&self, x: f32) -> f32 {
        self.corrected_x_to_norm(x / self.width_f32)
    }

    pub fn screen_y_to_norm(&self, y: f32) -> f32 {
        self.corrected_y_to_norm(y / self.height_f32)
    }

    pub fn norm_x_to_screen(&self, x: f32) -> f32 {
        self.norm_x_to_corrected(x) * self.width_f32
    }

    pub fn norm_y_to_screen(&self, y: f32) -> f32 {
        self.norm_y_to_corrected(y) * self.height_f32
    }

    pub fn constrain_resize(&self, new_width: u32, new_height: u32, _ratio: f32) -> (u32, u32) {
        let new_width_f32 = new_width as f32;
        let new_height_f32 = new_height as f32;
        let new_ratio = new_width_f32 / new_height_f32;

        let same_width = new_width == self.width_u32;
        let same_height = new_height == self.height_u32;

        log::info!(
            "width: {}, height: {} (ratio: {}) -> new_width: {}, new_height: {} (ratio: {}); same_width={}, same_height={}",
            self.width_u32,
            self.height_u32,
            self.ratio,
            new_width,
            new_height,
            new_ratio,
            same_width,
            same_height
        );

        // If the new ratio is greater, then we've expanded
        // our width farther; so we'll bump up the height to
        // preserve the ratio.
        // ratio = w / h
        // thus h = w / ratio
        let correct_height = !same_width || new_ratio > self.ratio;
        if correct_height {
            let corrected_height = (new_width_f32 / self.ratio).round() as u32;
            let (w, h) = (new_width, corrected_height);
            log::info!(
                "Corrected height: {}, new ratio: {}",
                corrected_height,
                w as f32 / h as f32
            );
            (w, h)
        } else {
            let corrected_width = (new_height_f32 * self.ratio).round() as u32;
            let (w, h) = (corrected_width, new_height);
            log::info!(
                "Corrected width: {}, new ratio: {}",
                corrected_width,
                w as f32 / h as f32
            );
            (w, h)
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub fn to_array3(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }
    pub fn to_array4(&self) -> [f32; 4] {
        [self.r, self.g, self.b, 1.0]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ShapeVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

impl ShapeVertex {
    pub fn new(position: [f32; 3], color: [f32; 3]) -> Self {
        Self { position, color }
    }
}

impl packed_shapes::Vertex for ShapeVertex {
    fn descriptor<'a>() -> wgpu::VertexBufferDescriptor<'a> {
        wgpu::VertexBufferDescriptor {
            stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float3,
                },
                wgpu::VertexAttributeDescriptor {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float3,
                },
            ],
        }
    }
}

pub struct ShapeVertexBuilder<'a> {
    pub color: [f32; 3],
    screen_metrics: &'a ScreenMetrics,
}

impl<'a> lyon::tessellation::StrokeVertexConstructor<ShapeVertex> for ShapeVertexBuilder<'a> {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> ShapeVertex {
        let position = vertex.position();
        let x = self.screen_metrics.norm_x_to_corrected(position.x) * 2.0 - 1.0;
        let y = self.screen_metrics.norm_y_to_corrected(position.y) * -2.0 + 1.0;
        ShapeVertex {
            position: [x, y, 0.0],
            color: self.color,
        }
    }
}

impl<'a> lyon::tessellation::FillVertexConstructor<ShapeVertex> for ShapeVertexBuilder<'a> {
    fn new_vertex(&mut self, vertex: FillVertex) -> ShapeVertex {
        let position = vertex.position();
        let x = self.screen_metrics.norm_x_to_corrected(position.x) * 2.0 - 1.0;
        let y = self.screen_metrics.norm_y_to_corrected(position.y) * -2.0 + 1.0;
        ShapeVertex {
            position: [x, y, 0.0],
            color: self.color,
        }
    }
}

#[derive(Debug)]
pub struct Arc {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub amount: f32,
    pub min_angle: f32,
    pub max_angle: f32,
    pub color: Color,
    pub stroke_width: f32,
    pub polarity: Polarity,
}

impl Arc {
    pub fn render(&self, screen_metrics: &ScreenMetrics) -> Buffers {
        let (x, y) = (self.x, self.y);

        // Build a Path.
        let mut builder = Path::svg_builder();
        let center: Point = point(x, y);

        let actual_radius = self.radius;
        let angle_range = self.max_angle - self.min_angle;
        let target_angle = self.amount * angle_range;

        let (start_angle, sweep_angle) = match &self.polarity {
            Polarity::Unipolar => (self.min_angle, target_angle),
            Polarity::Bipolar => {
                let midway = angle_range / 2.0;
                if target_angle < midway {
                    (target_angle + self.min_angle, midway - target_angle)
                } else {
                    (midway + self.min_angle, target_angle - midway)
                }
            }
        };

        let arc = lyon::geom::Arc {
            start_angle: -Angle::degrees(start_angle),
            center,
            radii: Vector::new(actual_radius, actual_radius),
            sweep_angle: -Angle::degrees(sweep_angle),
            x_rotation: Angle::degrees(180.0),
        };

        // If the current position is not on the arc, move or line to the beginning of the
        // arc.
        let arc_start = arc.from();
        builder.move_to(arc_start);

        arc.for_each_quadratic_bezier(&mut |curve| {
            builder.quadratic_bezier_to(curve.ctrl, curve.to);
        });

        let path = builder.build();
        // Create the destination vertex and index buffers.
        let mut buffers: Buffers = tessellation::VertexBuffers::new();

        {
            // Create the destination vertex and index buffers.
            let mut vertex_builder = BuffersBuilder::new(
                &mut buffers,
                ShapeVertexBuilder {
                    color: self.color.to_array3(),
                    screen_metrics,
                },
            );

            // Create the tessellator.
            let mut tessellator = StrokeTessellator::new();

            // Compute the tessellation.
            let opts = StrokeOptions::default()
                .with_tolerance(0.00005)
                .with_line_width(self.stroke_width)
                .with_line_join(LineJoin::MiterClip);
            tessellator
                .tessellate(&path, &opts, &mut vertex_builder)
                .unwrap();
        }

        buffers
    }
}

pub fn rectangle_outline(
    rect: &Rect,
    screen_metrics: &ScreenMetrics,
    stroke_width: f32,
    color: &[f32; 3],
) -> Buffers {
    let mut buffers: Buffers = tessellation::VertexBuffers::new();

    let mut stroke_tess = StrokeTessellator::new();

    let opts = StrokeOptions::default()
        .with_tolerance(0.00005)
        .with_line_width(stroke_width)
        .with_line_join(LineJoin::Round);
    let mut vertex_builder = BuffersBuilder::new(
        &mut buffers,
        ShapeVertexBuilder {
            color: *color,
            screen_metrics,
        },
    );
    stroke_tess
        .tessellate_polygon(
            Polygon {
                points: &[
                    point(rect.x1(), rect.y1()),
                    point(rect.x2(), rect.y1()),
                    point(rect.x2(), rect.y2()),
                    point(rect.x1(), rect.y2()),
                    point(rect.x1(), rect.y1()),
                ],
                closed: true,
            },
            &opts,
            &mut vertex_builder,
        )
        .unwrap();
    buffers
}

pub fn rectangle_solid(rect: &Rect, screen_metrics: &ScreenMetrics) -> Buffers {
    let box2d = Box2D::new(
        euclid::point2(rect.x1(), rect.y1()),
        euclid::point2(rect.x2(), rect.y2()),
    );

    let mut buffers: Buffers = tessellation::VertexBuffers::new();

    let mut fill_tess = FillTessellator::new();

    let opts = FillOptions::default();
    let mut vertex_builder = BuffersBuilder::new(
        &mut buffers,
        ShapeVertexBuilder {
            color: [1.0, 1.0, 1.0],
            screen_metrics,
        },
    );
    use lyon::tessellation::geometry_builder::*;
    fill_tess
        .tessellate_rectangle(&box2d.to_rect(), &opts, &mut vertex_builder)
        .unwrap();
    buffers
}

pub fn line_segment(
    from: &(f32, f32),
    to: &(f32, f32),
    screen_metrics: &ScreenMetrics,
    stroke_width: f32,
    color: &[f32; 3],
) -> Buffers {
    let mut buffers: Buffers = tessellation::VertexBuffers::new();

    let mut stroke_tess = StrokeTessellator::new();

    let opts = StrokeOptions::default()
        .with_tolerance(0.00005)
        .with_line_width(stroke_width)
        .with_line_join(LineJoin::Round);
    let mut vertex_builder = BuffersBuilder::new(
        &mut buffers,
        ShapeVertexBuilder {
            color: *color,
            screen_metrics,
        },
    );
    stroke_tess
        .tessellate_polygon(
            Polygon {
                points: &[point(from.0, from.1), point(to.0, to.1)],
                closed: false,
            },
            &opts,
            &mut vertex_builder,
        )
        .unwrap();
    buffers
}

pub fn ellipse_outline(rect: &Rect, screen_metrics: &ScreenMetrics, stroke_width: f32) -> Buffers {
    let mut buffers: Buffers = tessellation::VertexBuffers::new();

    let mut stroke_tess = StrokeTessellator::new();

    let opts = StrokeOptions::default()
        .with_tolerance(0.00005)
        .with_line_width(stroke_width)
        .with_line_join(LineJoin::Round);
    let mut vertex_builder = BuffersBuilder::new(
        &mut buffers,
        ShapeVertexBuilder {
            color: [1.0, 1.0, 1.0],
            screen_metrics,
        },
    );
    let center: Point = point(rect.mid_x(), rect.mid_y());
    let radii = vec2(rect.width() / 2.0, rect.height() / 2.0);
    stroke_tess
        .tessellate_ellipse(
            center,
            radii,
            Angle::degrees(0.0), // x_rotation
            lyon::path::Winding::Positive,
            &opts,
            &mut vertex_builder,
        )
        .unwrap();
    buffers
}

pub fn circle_outline(rect: &Rect, screen_metrics: &ScreenMetrics, stroke_width: f32) -> Buffers {
    let size = if rect.width() > rect.height() {
        rect.height()
    } else {
        rect.width()
    };
    let square = Rect::centered_at(rect.mid_x(), rect.mid_y(), size, size);
    ellipse_outline(&square, screen_metrics, stroke_width)
}
