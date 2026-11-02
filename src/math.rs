//! Small vector type. Uses Overwatch's axes: +Y is up, +Z is "Forward", +X is "Left".

use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    pub const UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    pub const FORWARD: Vec3 = Vec3::new(0.0, 0.0, 1.0);
    pub const LEFT: Vec3 = Vec3::new(1.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// `forward.cross(UP)` gives "right" in Overwatch's axes.
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn distance(self, o: Vec3) -> f32 {
        (self - o).length()
    }

    pub fn normalize_or_zero(self) -> Vec3 {
        let l = self.length();
        if l > 1e-6 {
            self * (1.0 / l)
        } else {
            Vec3::ZERO
        }
    }

    /// Same vector with the vertical part removed.
    pub fn horizontal(self) -> Vec3 {
        Vec3::new(self.x, 0.0, self.z)
    }

    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self + (o - self) * t
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Distance from `p` to segment `a..b`, and how far along the segment the closest point is (0..1).
pub fn point_segment_distance(p: Vec3, a: Vec3, b: Vec3) -> (f32, f32) {
    let ab = b - a;
    let len_sq = ab.dot(ab);
    let t = if len_sq > 1e-9 {
        ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((a + ab * t).distance(p), t)
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}
impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}
impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Vec3) {
        *self = *self - o;
    }
}
impl MulAssign<f32> for Vec3 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}
