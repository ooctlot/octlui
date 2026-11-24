//! Style enums shared by nodes and components.

/// Interaction state used to resolve a node's appearance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct InteractionState {
    /// The pointer is over the node or one of its descendants.
    pub hovered: bool,
    /// The node has keyboard focus in the active window.
    pub focused: bool,
    /// This is the innermost state-styled node under the primary mouse button when it was
    /// pressed. The state clears on release, window exit, focus loss or drag start.
    pub pressed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}
impl Axis {
    pub fn other(self) -> Axis {
        match self {
            Axis::X => Axis::Y,
            Axis::Y => Axis::X,
        }
    }
}

/// Alignment along one axis. `Start` is left/top.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}
impl Align {
    /// 0.0, 0.5 or 1.0
    #[inline]
    pub fn factor(self) -> f32 {
        match self {
            Align::Start => 0.0,
            Align::Center => 0.5,
            Align::End => 1.0,
        }
    }
}

/// Which side a scroll bar sits on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    /// Left (vertical bar) / top (horizontal bar).
    Start,
    /// Right (vertical bar) / bottom (horizontal bar).
    #[default]
    End,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    TopToBottom,
    BottomToTop,
    #[default]
    LeftToRight,
    RightToLeft,
}
impl Direction {
    pub fn axis(self) -> Axis {
        match self {
            Direction::TopToBottom | Direction::BottomToTop => Axis::Y,
            _ => Axis::X,
        }
    }
    pub fn reversed(self) -> bool {
        matches!(self, Direction::BottomToTop | Direction::RightToLeft)
    }
}

/// How a node is sized along one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SizeKind {
    /// As small as the content allows.
    Fit,
    /// Take the space left over by siblings (shared equally between all `Grow` siblings).
    Grow,
    /// Exactly this many logical pixels.
    Fixed(f32),
    /// A fraction (`0..=1`) of the parent's inner size.
    Fraction(f32),
}

/// A [`SizeKind`] with min/max constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizing {
    pub kind: SizeKind,
    pub min: f32,
    pub max: f32,
}
impl Default for Sizing {
    fn default() -> Self {
        fit()
    }
}
impl Sizing {
    pub const fn new(kind: SizeKind) -> Self {
        Self {
            kind,
            min: 0.0,
            max: f32::INFINITY,
        }
    }
    #[must_use]
    pub const fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }
    #[must_use]
    pub const fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }
    #[inline]
    pub fn clamp(&self, v: f32) -> f32 {
        v.max(self.min).min(self.max.max(self.min))
    }
}
impl From<SizeKind> for Sizing {
    fn from(k: SizeKind) -> Self {
        Sizing::new(k)
    }
}
/// A bare number means `Fixed`.
impl From<f32> for Sizing {
    fn from(v: f32) -> Self {
        fixed(v)
    }
}
pub const fn fit() -> Sizing {
    Sizing::new(SizeKind::Fit)
}
pub const fn grow() -> Sizing {
    Sizing::new(SizeKind::Grow)
}
pub const fn fixed(v: f32) -> Sizing {
    Sizing::new(SizeKind::Fixed(v))
}
pub const fn fraction(f: f32) -> Sizing {
    Sizing::new(SizeKind::Fraction(f))
}

/// Which axes a node clips (or scrolls) its content on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Clip {
    #[default]
    None,
    X,
    Y,
    XY,
}
impl Clip {
    pub fn x(self) -> bool {
        matches!(self, Clip::X | Clip::XY)
    }
    pub fn y(self) -> bool {
        matches!(self, Clip::Y | Clip::XY)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TextWrap {
    /// Single line, no wrapping or clipping. Overflows the node.
    None,
    /// Break between any two letters.
    Letters,
    /// Break at word boundaries (default).
    #[default]
    Words,
    /// Only break at explicit `\n`.
    NewLines,
    /// Single line, clipped at the node bounds, optionally ending in `...`.
    Clip { ellipsis: bool },
    /// Single line; the last `length` logical pixels fade out.
    Fade { length: f32 },
}
impl TextWrap {
    pub fn single_line(self) -> bool {
        matches!(
            self,
            TextWrap::None | TextWrap::Clip { .. } | TextWrap::Fade { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFit {
    /// Scale uniformly so the whole image is visible.
    #[default]
    Contain,
    /// Scale uniformly so the node is fully covered (cropping).
    Cover,
    /// Stretch to the node's size.
    Fill,
    /// Natural size, centered, cropped to the node.
    None,
}
