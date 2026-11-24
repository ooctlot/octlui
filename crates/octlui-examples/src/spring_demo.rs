use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    components::{Floating, Image, Spring, SpringProps, Text},
    geometry::{Anchor, Color, Fill, Vec2},
    node,
    style::*,
};

#[derive(Clone, Copy)]
pub(crate) enum Target {
    Position,
    VisualSize,
    Width,
    Height,
    Padding,
    ChildGap,
    Background,
    BorderPaint,
    BorderWidth,
    CornerRadius,
    TextColor,
    ImageTint,
    FloatingOffset,
}

pub(crate) fn demo(state: &Gallery, cx: &Cx, target: Target, description: &str) -> Node {
    let props = match target {
        Target::Position => SpringProps::POSITION,
        Target::VisualSize => SpringProps::SIZE,
        Target::Width => SpringProps::WIDTH,
        Target::Height => SpringProps::HEIGHT,
        Target::Padding => SpringProps::PADDING,
        Target::ChildGap => SpringProps::CHILD_GAP,
        Target::Background => SpringProps::BACKGROUND,
        Target::BorderPaint => SpringProps::BORDER,
        Target::BorderWidth => SpringProps::BORDER_WIDTH,
        Target::CornerRadius => SpringProps::CORNER_RADIUS,
        Target::TextColor => SpringProps::TEXT_COLOR,
        Target::ImageTint => SpringProps::TINT,
        Target::FloatingOffset => SpringProps::FLOATING_OFFSET,
    };
    let spring = if state.bouncy {
        Spring::bouncy(props)
    } else {
        Spring::new(props)
    };
    let large = state.spring_step != 0;
    let fill = [
        Fill::solid(0x81b8ae),
        Fill::linear(0.4, &[Color::hex(0xe5b35f), Color::hex(0xa75b5a)]),
        Fill::radial(&[Color::hex(0xa4d99d), Color::hex(0x5e79cd)]),
    ][state.spring_step];
    let target_id = ("spring-demo", target as usize);
    let sample = || {
        node()
            .id(target_id)
            .column()
            .size(205., 108.)
            .padding(10.)
            .child_gap(6.)
            .corner_radius(8.)
            .background(0xc3e3da)
    };
    let child = |text| {
        label(text, 15., INK)
            .padding(5.)
            .background(CARD)
            .corner_radius(4.)
    };
    let sibling = || child("Sibling").size(95., 35.).background(0xd5e8ee);
    let scene = match target {
        Target::Position => row()
            .size(grow(), 120.)
            .child(node().width(if large { 140. } else { 0. }))
            .child(
                sample()
                    .child(child("Moves with its parent"))
                    .spring(spring),
            ),
        Target::VisualSize => row()
            .size(grow(), 155.)
            .child(
                sample()
                    .size(
                        if large { 245. } else { 145. },
                        if large { 140. } else { 95. },
                    )
                    .child(child("Visual bounds"))
                    .spring(spring),
            )
            .child(sibling()),
        Target::Width => row()
            .child(
                sample()
                    .width(if large { 245. } else { 145. })
                    .spring(spring),
            )
            .child(sibling()),
        Target::Height => node()
            .column()
            .width(grow())
            .child_gap(8.)
            .child(
                sample()
                    .height(if large { 145. } else { 70. })
                    .spring(spring),
            )
            .child(sibling()),
        Target::Padding => sample()
            .height(145.)
            .padding(if large { 28. } else { 8. })
            .children([child("First"), child("Second")])
            .spring(spring),
        Target::ChildGap => sample()
            .height(145.)
            .child_gap(if large { 36. } else { 5. })
            .children([child("First"), child("Second")])
            .spring(spring),
        Target::Background => sample().background(fill).spring(spring),
        Target::BorderPaint => sample().border(fill, 9.).spring(spring),
        Target::BorderWidth => sample()
            .border(ACCENT, if large { 15. } else { 2. })
            .spring(spring),
        Target::CornerRadius => sample()
            .corner_radius(if large { [38., 6., 24., 3.] } else { [3.; 4] })
            .spring(spring),
        Target::TextColor => sample()
            .background(NAV)
            .text(Text::new("Animated text").size(24.).color(fill))
            .spring(spring),
        Target::ImageTint => {
            if let Some(id) = state.image {
                sample()
                    .image(Image::new(id).fit(ImageFit::Cover).tint(fill))
                    .spring(spring)
            } else {
                note("The registered image is unavailable.")
            }
        }
        Target::FloatingOffset => node().height(145.).child(
            sample()
                .child(
                    child("Badge")
                        .id(target_id)
                        .width(fit())
                        .background(ACCENT)
                        .text(Text::new("Badge").color(0xffffff))
                        .floating(
                            Floating::floating()
                                .parent_anchor(Anchor::BOTTOM_LEFT)
                                .anchor(Anchor::CENTER_LEFT)
                                .offset(Vec2::new(if large { 140. } else { 10. }, -10.)),
                        )
                        .spring(spring),
                )
                .id("spring-anchor"),
        ),
    };
    page(description)
        .child(row().children([
            control("Next target", cx, move |s| {
                let variants = match target {
                    Target::Background
                    | Target::BorderPaint
                    | Target::TextColor
                    | Target::ImageTint => 3,
                    _ => 2,
                };
                s.spring_step = (s.spring_step + 1) % variants;
            }),
            control(
                if state.bouncy {
                    "Bouncy"
                } else {
                    "Critically damped"
                },
                cx,
                |s| s.bouncy = !s.bouncy,
            ),
        ]))
        .child(card("Live preview").child(scene))
}
