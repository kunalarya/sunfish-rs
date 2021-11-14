use serde::Deserialize;

#[derive(Clone, Debug)]
pub enum UserVec2 {
    // Screen position, relative -- 0.0 to 1.0, with origin in the top-left corner.
    Rel(Vec2),
}

impl UserVec2 {
    pub fn unpack(&self) -> (f32, f32) {
        match self {
            UserVec2::Rel(Vec2 { pos }) => (pos[0], pos[1]),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Rect {
    pub pos: [f32; 4],
}

impl Rect {
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Rect {
        Rect {
            pos: [x1, y1, x2, y2],
        }
    }

    pub fn centered_at(mid_x: f32, mid_y: f32, width: f32, height: f32) -> Rect {
        let x1 = mid_x - (width / 2.0);
        let x2 = mid_x + (width / 2.0);
        let y1 = mid_y - (height / 2.0);
        let y2 = mid_y + (height / 2.0);
        Self::new(x1, y1, x2, y2)
    }

    pub fn in_bounds(&self, x: f32, y: f32) -> bool {
        x >= self.pos[0] && y >= self.pos[1] && x <= self.pos[2] && y <= self.pos[3]
    }

    pub fn size(&self) -> [f32; 2] {
        [self.pos[2] - self.pos[0], self.pos[3] - self.pos[1]]
    }

    pub fn width(&self) -> f32 {
        self.x2() - self.x1()
    }

    pub fn height(&self) -> f32 {
        self.y2() - self.y1()
    }

    pub fn x1(&self) -> f32 {
        self.pos[0]
    }

    pub fn y1(&self) -> f32 {
        self.pos[1]
    }

    pub fn x2(&self) -> f32 {
        self.pos[2]
    }

    pub fn y2(&self) -> f32 {
        self.pos[3]
    }

    pub fn mid_x(&self) -> f32 {
        self.pos[0] + (self.width() / 2.0)
    }

    pub fn mid_y(&self) -> f32 {
        self.pos[1] + (self.height() / 2.0)
    }

    pub fn to_lyon_rect(&self) -> lyon::path::math::Rect {
        use lyon::geom::point;
        use lyon::geom::size;
        lyon::path::math::Rect::new(
            point(0.5, 0.5),
            size(0.25, 0.25),
            // point(self.pos[0], self.pos[1]),
            // size(self.width(), self.height()),
        )
    }

    /// Contract the rectangle by the given percent.
    pub fn contract(&self, amount_pct: f32) -> Self {
        let amount_pct = amount_pct.min(1.0).max(0.0);
        let trim_horizontal = (self.width() * (1.0 - amount_pct)) / 2.0;
        let trim_vertical = (self.height() * (1.0 - amount_pct)) / 2.0;
        Rect {
            pos: [
                self.x1() + trim_horizontal,
                self.y1() + trim_vertical,
                self.x2() - trim_horizontal,
                self.y2() - trim_vertical,
            ],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Vec2 {
    pub pos: [f32; 2],
}

#[derive(PartialEq, Clone, Debug)]
pub struct Coord2 {
    pub x: f32,
    pub y: f32,
}

impl Coord2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}
