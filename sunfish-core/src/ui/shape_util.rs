use lyon::geom::euclid;
use lyon::geom::euclid::{vec2, Box2D};
use lyon::math::{point, Angle, Point, Vector};
use lyon::path::builder::Build;
use lyon::path::Path;
use lyon::path::Polygon;
use lyon::tessellation::{
    self, BuffersBuilder, FillOptions, FillTessellator, LineJoin, StrokeOptions, StrokeTessellator,
};

use crate::ui::coords::Rect;
use crate::ui::shapes::{Buffers, Color, Polarity, ScreenMetrics, ShapeVertexBuilder};

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
