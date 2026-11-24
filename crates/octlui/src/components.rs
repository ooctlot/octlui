//! Optional node components: appearance, text, image, tooltip, scroll, floating, spring.

use crate::anim::SpringParams;
use crate::geometry::{Anchor, Color, Fill, Vec2};
use crate::image::ImageId;
use crate::node::Node;
use crate::style::{Align, Clip, ImageFit, Side, TextWrap};
use std::sync::Arc;
use std::time::Duration;

/// Background and border.
#[derive(Clone, Debug, Default, PartialEq)]
#[octlui::builder]
pub struct Appearance {
    #[octlui::property(into)]
    pub background: Fill,
    #[octlui::property(into)]
    pub border_color: Fill,
    /// Border width in logical pixels (drawn inside the node's bounds).
    #[octlui::property]
    pub border_width: f32,
}

/// Font handle. Use a built-in family or load a face through `AppContext`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FontId(pub u32);
impl FontId {
    pub const DEFAULT: FontId = FontId(0);
    pub const SERIF: FontId = FontId(1);
    pub const MONOSPACE: FontId = FontId(2);
}

/// Text content of a node. The node becomes a text leaf: its children are ignored and its
/// size derives from the measured text.
#[derive(Clone, Debug, PartialEq)]
#[octlui::builder(no_constructor)]
pub struct Text {
    #[octlui::property(into)]
    pub content: Arc<str>,
    #[octlui::property(into)]
    pub color: Fill,
    /// Font size in logical pixels.
    #[octlui::property]
    pub size: f32,
    #[octlui::property]
    pub font: FontId,
    /// Horizontal alignment of the text inside the node.
    #[octlui::property]
    pub align: Align,
    #[octlui::property]
    pub wrap: TextWrap,
}
impl Text {
    pub fn new(content: impl Into<Arc<str>>) -> Self {
        Self {
            content: content.into(),
            color: Fill::Solid(Color::hex(0xe0e0e0)),
            size: 16.0,
            font: FontId::DEFAULT,
            align: Align::Start,
            wrap: TextWrap::Words,
        }
    }
}
impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Text::new(s)
    }
}
impl From<String> for Text {
    fn from(s: String) -> Self {
        Text::new(s)
    }
}

/// An image drawn above the background and below text.
#[derive(Clone, Debug, PartialEq)]
#[octlui::builder(no_constructor)]
pub struct Image {
    #[octlui::property]
    pub source: ImageId,
    /// Multiplied with the image colors (solid or gradient).
    #[octlui::property(into)]
    pub tint: Fill,
    #[octlui::property]
    pub fit: ImageFit,
}
impl Image {
    pub fn new(source: ImageId) -> Self {
        Self { source, tint: Fill::Solid(Color::WHITE), fit: ImageFit::Contain }
    }
}
impl From<ImageId> for Image {
    fn from(id: ImageId) -> Self {
        Image::new(id)
    }
}

/// A node shown near its owner after the pointer rests on it.
#[derive(Clone)]
#[octlui::builder(no_constructor)]
pub struct Tooltip {
    #[octlui::property]
    pub content: Node,
    /// How long the pointer must rest on the owner before the tooltip appears.
    #[octlui::property]
    pub delay: Duration,
    #[octlui::property]
    pub fade_in: Duration,
    #[octlui::property]
    pub fade_out: Duration,
    /// Gap between the owner and the tooltip.
    #[octlui::property]
    pub gap: f32,
}
impl Tooltip {
    pub fn new(content: Node) -> Self {
        Self {
            content,
            delay: Duration::from_millis(500),
            fade_in: Duration::from_millis(150),
            fade_out: Duration::from_millis(100),
            gap: 6.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[octlui::builder]
pub struct ScrollBar {
    #[octlui::property]
    pub corner_radius: f32,
    #[octlui::property(into)]
    pub bar_color: Fill,
    #[octlui::property(into)]
    pub background_color: Fill,
    /// Shrink the node's content area by the bar thickness so the bar never overlaps content.
    #[octlui::property]
    pub reserve_space: bool,
    #[octlui::property]
    pub side: Side,
    /// If `false` the bar fades in while the pointer is over the node (or while scrolling or
    /// dragging) and fades out afterwards.
    #[octlui::property]
    pub always_visible: bool,
    #[octlui::property]
    pub thickness: f32,
    #[octlui::property]
    pub fade_in: Duration,
    #[octlui::property]
    pub fade_out: Duration,
    #[octlui::property]
    pub min_thumb_length: f32,
}
impl Default for ScrollBar {
    fn default() -> Self {
        Self {
            corner_radius: 4.0,
            bar_color: Fill::Solid(Color::WHITE.with_alpha(0.35)),
            background_color: Fill::NONE,
            reserve_space: false,
            side: Side::End,
            always_visible: false,
            thickness: 8.0,
            fade_in: Duration::from_millis(120),
            fade_out: Duration::from_millis(300),
            min_thumb_length: 24.0,
        }
    }
}

/// Makes a node scrollable. Children are clipped on the scrolled axes.
#[derive(Clone, Debug, PartialEq)]
#[octlui::builder]
pub struct Scroll {
    /// Axes that scroll (`Clip::None` disables scrolling).
    #[octlui::property]
    pub axes: Clip,
    /// Child offset in logical pixels (positive = content moved up/left). When the declared
    /// value changes between frames the scroll position jumps (or glides, if `smooth`) there.
    #[octlui::property]
    pub offset: Option<Vec2>,
    /// Glide towards the target offset instead of jumping.
    #[octlui::property]
    pub smooth: bool,
    /// Approach rate of smooth scrolling in 1/s (higher = snappier).
    #[octlui::property]
    pub smooth_rate: f32,
    /// React to mouse wheel / track pad scrolling.
    #[octlui::property]
    pub wheel: bool,
    #[octlui::property]
    pub scroll_bar: Option<ScrollBar>,
}
impl Default for Scroll {
    fn default() -> Self {
        Self { axes: Clip::Y, offset: None, smooth: true, smooth_rate: 18.0, wheel: true, scroll_bar: None }
    }
}

/// Lifts a node out of the layout flow, attaching it to its parent.
#[derive(Clone, Debug, PartialEq)]
#[octlui::builder]
pub struct Floating {
    /// Point on the parent the node attaches to.
    #[octlui::property]
    pub parent_anchor: Anchor,
    /// Point on the floating node placed on that parent point.
    #[octlui::property]
    pub anchor: Anchor,
    /// Extra offset in logical pixels.
    #[octlui::property]
    pub offset: Vec2,
    /// Clip the node to its parent's bounds.
    #[octlui::property]
    pub clip_to_parent: bool,
    /// Draw order among floating nodes (higher is on top).
    #[octlui::property]
    pub z_index: i16,
}
impl Default for Floating {
    fn default() -> Self {
        Self {
            parent_anchor: Anchor::TOP_LEFT,
            anchor: Anchor::TOP_LEFT,
            offset: Vec2::ZERO,
            clip_to_parent: false,
            z_index: 0,
        }
    }
}

/// Set of node properties a [`Spring`] animates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpringProps(u16);
impl SpringProps {
    pub const NONE: SpringProps = SpringProps(0);
    /// Moves the node and its whole subtree.
    pub const POSITION: SpringProps = SpringProps(1);
    /// Size of the node's own background/border/image (children keep their layout).
    pub const SIZE: SpringProps = SpringProps(2);
    pub const BACKGROUND: SpringProps = SpringProps(4);
    pub const BORDER: SpringProps = SpringProps(8);
    pub const TINT: SpringProps = SpringProps(16);
    pub const TEXT_COLOR: SpringProps = SpringProps(32);
    /// Resolved width, including sibling and child reflow.
    pub const WIDTH: SpringProps = SpringProps(1 << 6);
    /// Resolved height, including sibling and child reflow.
    pub const HEIGHT: SpringProps = SpringProps(1 << 7);
    pub const PADDING: SpringProps = SpringProps(1 << 8);
    pub const CHILD_GAP: SpringProps = SpringProps(1 << 9);
    pub const CORNER_RADIUS: SpringProps = SpringProps(1 << 10);
    pub const BORDER_WIDTH: SpringProps = SpringProps(1 << 11);
    pub const FLOATING_OFFSET: SpringProps = SpringProps(1 << 12);
    /// Animates width, height, padding and child gap with reflow.
    pub const LAYOUT: SpringProps = SpringProps(
        Self::WIDTH.0 | Self::HEIGHT.0 | Self::PADDING.0 | Self::CHILD_GAP.0,
    );
    /// Original visual spring properties, including position, size and paints.
    pub const ALL: SpringProps = SpringProps(63);
    pub const fn contains(self, o: SpringProps) -> bool {
        self.0 & o.0 == o.0 && o.0 != 0
    }
    pub(crate) const fn intersects(self, o: SpringProps) -> bool {
        self.0 & o.0 != 0
    }
}
impl std::ops::BitOr for SpringProps {
    type Output = SpringProps;
    fn bitor(self, o: SpringProps) -> SpringProps {
        SpringProps(self.0 | o.0)
    }
}

/// Makes the chosen properties lag behind their declared values like a damped spring.
///
/// Width, height, padding and child gap can reflow layout. The other properties affect
/// presentation. Combine properties with `|` to use the same spring parameters for them.
#[derive(Clone, Debug, PartialEq)]
#[octlui::builder(no_constructor)]
pub struct Spring {
    #[octlui::property]
    pub properties: SpringProps,
    #[octlui::property]
    pub stiffness: f32,
    #[octlui::property]
    pub damping: f32,
    #[octlui::property]
    pub mass: f32,
    #[octlui::property]
    pub precision: f32,
}
impl Spring {
    /// Critically damped spring (no overshoot).
    pub fn new(properties: SpringProps) -> Self {
        let p = SpringParams::default();
        Self { properties, stiffness: p.stiffness, damping: p.damping, mass: p.mass, precision: p.precision }
    }
    /// Lightly damped spring that overshoots and wobbles.
    pub fn bouncy(properties: SpringProps) -> Self {
        Self { stiffness: 400.0, damping: 18.0, ..Self::new(properties) }
    }
    pub fn params(&self) -> SpringParams {
        SpringParams {
            stiffness: self.stiffness,
            damping: self.damping,
            mass: self.mass.max(0.0001),
            precision: self.precision,
        }
    }
}
