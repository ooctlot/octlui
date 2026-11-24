//! Basic geometry and color types.

use crate::style::Align;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[inline]
pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

impl Vec2 {
    pub const ZERO: Vec2 = vec2(0.0, 0.0);
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    #[inline]
    pub fn get(self, axis: crate::style::Axis) -> f32 {
        match axis {
            crate::style::Axis::X => self.x,
            crate::style::Axis::Y => self.y,
        }
    }
    #[inline]
    pub fn set(&mut self, axis: crate::style::Axis, v: f32) {
        match axis {
            crate::style::Axis::X => self.x = v,
            crate::style::Axis::Y => self.y = v,
        }
    }
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn lerp(self, o: Vec2, t: f32) -> Vec2 {
        self + (o - self) * t
    }
}
impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        vec2(self.x + o.x, self.y + o.y)
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        *self = *self + o;
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        vec2(self.x - o.x, self.y - o.y)
    }
}
impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Vec2) {
        *self = *self - o;
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        vec2(self.x * s, self.y * s)
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        vec2(-self.x, -self.y)
    }
}
impl From<(f32, f32)> for Vec2 {
    fn from((x, y): (f32, f32)) -> Self {
        vec2(x, y)
    }
}
impl From<[f32; 2]> for Vec2 {
    fn from([x, y]: [f32; 2]) -> Self {
        vec2(x, y)
    }
}

/// Axis-aligned rectangle (origin + size), in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// A rect that contains every point; used as "no clip".
    pub const INFINITE: Rect = Rect { x: -1.0e9, y: -1.0e9, w: 2.0e9, h: 2.0e9 };

    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn from_origin_size(o: Vec2, s: Vec2) -> Self {
        Self::new(o.x, o.y, s.x, s.y)
    }
    #[inline]
    pub fn origin(&self) -> Vec2 {
        vec2(self.x, self.y)
    }
    #[inline]
    pub fn size(&self) -> Vec2 {
        vec2(self.w, self.h)
    }
    #[inline]
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    #[inline]
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    #[inline]
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.right() && p.y < self.bottom()
    }
    pub fn translate(&self, d: Vec2) -> Rect {
        Rect::new(self.x + d.x, self.y + d.y, self.w, self.h)
    }
    /// Intersection; returns a zero-sized rect when disjoint.
    pub fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
    }
    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
    pub fn center(&self) -> Vec2 {
        vec2(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
    pub fn shrink(&self, p: Padding) -> Rect {
        Rect::new(
            self.x + p.left,
            self.y + p.top,
            (self.w - p.left - p.right).max(0.0),
            (self.h - p.top - p.bottom).max(0.0),
        )
    }
    /// The point of this rect described by `anchor`.
    pub fn anchor_point(&self, a: Anchor) -> Vec2 {
        vec2(self.x + self.w * a.x.factor(), self.y + self.h * a.y.factor())
    }
}

/// 2D anchor: a point on a rectangle described by two [`Align`] values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Anchor {
    pub x: Align,
    pub y: Align,
}
impl Anchor {
    pub const TOP_LEFT: Anchor = Anchor { x: Align::Start, y: Align::Start };
    pub const TOP_CENTER: Anchor = Anchor { x: Align::Center, y: Align::Start };
    pub const TOP_RIGHT: Anchor = Anchor { x: Align::End, y: Align::Start };
    pub const CENTER_LEFT: Anchor = Anchor { x: Align::Start, y: Align::Center };
    pub const CENTER: Anchor = Anchor { x: Align::Center, y: Align::Center };
    pub const CENTER_RIGHT: Anchor = Anchor { x: Align::End, y: Align::Center };
    pub const BOTTOM_LEFT: Anchor = Anchor { x: Align::Start, y: Align::End };
    pub const BOTTOM_CENTER: Anchor = Anchor { x: Align::Center, y: Align::End };
    pub const BOTTOM_RIGHT: Anchor = Anchor { x: Align::End, y: Align::End };
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Padding {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
impl Padding {
    pub const ZERO: Padding = Padding { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 };
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self { left, top, right, bottom }
    }
    pub const fn all(v: f32) -> Self {
        Self::new(v, v, v, v)
    }
    pub const fn xy(x: f32, y: f32) -> Self {
        Self::new(x, y, x, y)
    }
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}
impl From<f32> for Padding {
    fn from(v: f32) -> Self {
        Self::all(v)
    }
}
/// `(horizontal, vertical)`
impl From<(f32, f32)> for Padding {
    fn from((x, y): (f32, f32)) -> Self {
        Self::xy(x, y)
    }
}
/// `[left, top, right, bottom]`
impl From<[f32; 4]> for Padding {
    fn from([l, t, r, b]: [f32; 4]) -> Self {
        Self::new(l, t, r, b)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}
impl CornerRadius {
    pub const fn all(r: f32) -> Self {
        Self { top_left: r, top_right: r, bottom_right: r, bottom_left: r }
    }
    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self { top_left, top_right, bottom_right, bottom_left }
    }
    pub fn is_zero(&self) -> bool {
        self.top_left == 0.0 && self.top_right == 0.0 && self.bottom_right == 0.0 && self.bottom_left == 0.0
    }
}
impl From<f32> for CornerRadius {
    fn from(r: f32) -> Self {
        Self::all(r)
    }
}
/// `[top_left, top_right, bottom_right, bottom_left]`
impl From<[f32; 4]> for CornerRadius {
    fn from([a, b, c, d]: [f32; 4]) -> Self {
        Self::new(a, b, c, d)
    }
}

/// Non-premultiplied sRGB color with float channels in `0..=1`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
impl Color {
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Color = Color::rgba(0.0, 0.0, 0.0, 1.0);
    pub const WHITE: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::rgba(r, g, b, 1.0)
    }
    /// From `0xRRGGBB`.
    pub const fn hex(rgb: u32) -> Self {
        Self::rgba(
            ((rgb >> 16) & 0xff) as f32 / 255.0,
            ((rgb >> 8) & 0xff) as f32 / 255.0,
            (rgb & 0xff) as f32 / 255.0,
            1.0,
        )
    }
    /// From `0xRRGGBBAA`.
    pub const fn hex_a(rgba: u32) -> Self {
        Self::rgba(
            ((rgba >> 24) & 0xff) as f32 / 255.0,
            ((rgba >> 16) & 0xff) as f32 / 255.0,
            ((rgba >> 8) & 0xff) as f32 / 255.0,
            (rgba & 0xff) as f32 / 255.0,
        )
    }
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
    pub fn mul_alpha(self, k: f32) -> Self {
        Self { a: self.a * k, ..self }
    }
    pub fn lerp(self, o: Color, t: f32) -> Color {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Color::rgba(l(self.r, o.r), l(self.g, o.g), l(self.b, o.b), l(self.a, o.a))
    }
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
    pub fn from_array([r, g, b, a]: [f32; 4]) -> Color {
        Color::rgba(r, g, b, a)
    }
}
/// `0xRRGGBB`
impl From<u32> for Color {
    fn from(v: u32) -> Self {
        Color::hex(v)
    }
}
impl From<[f32; 4]> for Color {
    fn from(v: [f32; 4]) -> Self {
        Color::from_array(v)
    }
}

/// Maximum number of stops in a gradient.
pub const MAX_GRADIENT_STOPS: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColorStop {
    pub offset: f32,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stops {
    len: u8,
    items: [ColorStop; MAX_GRADIENT_STOPS],
}
impl Stops {
    pub fn new(stops: &[ColorStop]) -> Self {
        let mut items = [ColorStop::default(); MAX_GRADIENT_STOPS];
        let n = stops.len().min(MAX_GRADIENT_STOPS);
        items[..n].copy_from_slice(&stops[..n]);
        Self { len: n as u8, items }
    }
    pub fn as_slice(&self) -> &[ColorStop] {
        &self.items[..self.len as usize]
    }
    pub fn len(&self) -> usize {
        self.len as usize
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A fill: solid color or gradient (at most [`MAX_GRADIENT_STOPS`] stops).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    Solid(Color),
    /// `angle` in radians: `0` = left to right, `PI/2` = top to bottom.
    Linear { angle: f32, stops: Stops },
    /// Centered on the node, `t = 1` at the farthest corner.
    Radial { stops: Stops },
}
impl Default for Fill {
    fn default() -> Self {
        Fill::Solid(Color::TRANSPARENT)
    }
}
impl Fill {
    pub const NONE: Fill = Fill::Solid(Color::TRANSPARENT);

    pub fn solid(c: impl Into<Color>) -> Fill {
        Fill::Solid(c.into())
    }
    /// Evenly spaced linear gradient between `colors`.
    pub fn linear(angle: f32, colors: &[Color]) -> Fill {
        Fill::Linear { angle, stops: even_stops(colors) }
    }
    pub fn linear_stops(angle: f32, stops: &[ColorStop]) -> Fill {
        Fill::Linear { angle, stops: Stops::new(stops) }
    }
    pub fn radial(colors: &[Color]) -> Fill {
        Fill::Radial { stops: even_stops(colors) }
    }
    pub fn is_visible(&self) -> bool {
        match self {
            Fill::Solid(c) => c.a > 0.0,
            _ => true,
        }
    }
    pub fn as_solid(&self) -> Option<Color> {
        match self {
            Fill::Solid(c) => Some(*c),
            _ => None,
        }
    }

    /// Interpolates gradient stops and direction. A change between linear and radial
    /// collapses to a solid at the midpoint before expanding into the new gradient.
    pub(crate) fn interpolate(self, to: Self, t: f32) -> Self {
        if t <= 0. {
            return self;
        }
        if t >= 1. {
            return to;
        }
        match (self, to) {
            (Self::Solid(a), Self::Solid(b)) => Self::Solid(a.lerp(b, t)),
            (Self::Linear { angle: a, stops: from }, Self::Linear { angle: b, stops: to }) => {
                let angle = a + ((b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI) * t;
                Self::Linear { angle, stops: interpolate_stops(from, to, t) }
            }
            (Self::Radial { stops: a }, Self::Radial { stops: b }) => {
                Self::Radial { stops: interpolate_stops(a, b, t) }
            }
            (Self::Solid(c), Self::Linear { angle, stops }) => Self::Linear {
                angle,
                stops: interpolate_stops(solid_stops(c, stops), stops, t),
            },
            (Self::Solid(c), Self::Radial { stops }) => Self::Radial {
                stops: interpolate_stops(solid_stops(c, stops), stops, t),
            },
            (_, Self::Solid(_)) => to.interpolate(self, 1. - t),
            (Self::Linear { stops: a, .. } | Self::Radial { stops: a },
             Self::Linear { stops: b, .. } | Self::Radial { stops: b }) => {
                let midpoint = Self::Solid(sample_stops(a, 0.5).lerp(sample_stops(b, 0.5), 0.5));
                if t < 0.5 {
                    self.interpolate(midpoint, 2. * t)
                } else {
                    midpoint.interpolate(to, 2. * t - 1.)
                }
            }
        }
    }
    pub fn mul_alpha(&self, k: f32) -> Fill {
        let mut f = *self;
        match &mut f {
            Fill::Solid(c) => *c = c.mul_alpha(k),
            Fill::Linear { stops, .. } | Fill::Radial { stops } => {
                let n = stops.len();
                for s in &mut stops.items[..n] {
                    s.color = s.color.mul_alpha(k);
                }
            }
        }
        f
    }
}

fn solid_stops(color: Color, mut stops: Stops) -> Stops {
    if stops.is_empty() {
        return Stops::new(&[ColorStop { offset: 0., color }]);
    }
    let len = stops.len();
    for stop in &mut stops.items[..len] {
        stop.color = color;
    }
    stops
}

fn interpolate_stops(a: Stops, b: Stops, t: f32) -> Stops {
    let len = a.len().max(b.len()).max(1);
    let last = |stops: &Stops, i: usize| {
        let items = stops.as_slice();
        items.get(i).or_else(|| items.last()).copied().unwrap_or_default()
    };
    let mut stops = [ColorStop::default(); MAX_GRADIENT_STOPS];
    for (i, stop) in stops[..len].iter_mut().enumerate() {
        let (a, b) = (last(&a, i), last(&b, i));
        *stop = ColorStop {
            offset: a.offset + (b.offset - a.offset) * t,
            color: a.color.lerp(b.color, t),
        };
    }
    Stops::new(&stops[..len])
}

fn sample_stops(stops: Stops, t: f32) -> Color {
    let stops = stops.as_slice();
    let mut color = stops.first().map_or(Color::TRANSPARENT, |s| s.color);
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if t > a.offset {
            color = a.color.lerp(b.color, ((t - a.offset) / (b.offset - a.offset).max(0.00001)).clamp(0., 1.));
        }
    }
    color
}
fn even_stops(colors: &[Color]) -> Stops {
    let n = colors.len().min(MAX_GRADIENT_STOPS);
    let mut v = [ColorStop::default(); MAX_GRADIENT_STOPS];
    for (i, c) in colors.iter().take(n).enumerate() {
        v[i] = ColorStop { offset: if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 }, color: *c };
    }
    Stops::new(&v[..n])
}
impl From<Color> for Fill {
    fn from(c: Color) -> Self {
        Fill::Solid(c)
    }
}
/// `0xRRGGBB`
impl From<u32> for Fill {
    fn from(c: u32) -> Self {
        Fill::Solid(Color::hex(c))
    }
}
impl From<[f32; 4]> for Fill {
    fn from(c: [f32; 4]) -> Self {
        Fill::Solid(Color::from_array(c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rect_ops() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersect(&b), Rect::new(5.0, 5.0, 5.0, 5.0));
        assert!(a.contains(vec2(0.0, 0.0)) && !a.contains(vec2(10.0, 5.0)));
        assert_eq!(a.anchor_point(Anchor::BOTTOM_RIGHT), vec2(10.0, 10.0));
    }
    #[test]
    fn hex() {
        assert_eq!(Color::hex(0xff0000), Color::rgb(1.0, 0.0, 0.0));
    }
}
