use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    components::{Floating, Spring, SpringProps},
    geometry::{Anchor, Color, Fill, Vec2},
    node,
    style::*,
};

pub(crate) const CATEGORY: &str = "Motion";
pub(crate) const TITLE: &str = "Layout and paint springs";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let (width, height, padding, gap, corner, border, offset, fill) = [
        (
            155.,
            124.,
            8.,
            4.,
            8.,
            2.,
            Vec2::new(-30., 14.),
            Fill::solid(0xc3e3da),
        ),
        (
            255.,
            152.,
            18.,
            12.,
            13.,
            6.,
            Vec2::new(-12., 16.),
            Fill::linear(0.4, &[Color::hex(0xf4ddaa), Color::hex(0xd99676)]),
        ),
        (
            204.,
            178.,
            24.,
            8.,
            20.,
            3.,
            Vec2::new(-55., 30.),
            Fill::radial(&[
                Color::hex(0xe4efba),
                Color::hex(0x76aea9),
                Color::hex(0xa0bcdc),
            ]),
        ),
    ][state.spring_step];
    let motion = |properties| {
        if state.bouncy {
            Spring::bouncy(properties)
        } else {
            Spring::new(properties)
        }
    };
    let moving = node()
        .column()
        .id("spring-target")
        .size(width, height)
        .padding(padding)
        .child_gap(gap)
        .corner_radius(corner)
        .border(ACCENT, border)
        .background(fill)
        .spring(motion(
            SpringProps::LAYOUT
                | SpringProps::BACKGROUND
                | SpringProps::BORDER_WIDTH
                | SpringProps::CORNER_RADIUS,
        ))
        .children(["Reflowing", "children"].map(|text| {
            label(text, 15., INK)
                .padding((6., 3.))
                .background(0xf5faf9)
                .corner_radius(4.)
        }))
        .child(
            label("Anchor", 12., 0xf4f8f7)
                .width(fit())
                .padding(5.)
                .background(ACCENT)
                .corner_radius(4.)
                .floating(
                    Floating::floating()
                        .parent_anchor(Anchor::BOTTOM_RIGHT)
                        .anchor(Anchor::CENTER)
                        .offset(offset),
                )
                .spring(motion(SpringProps::FLOATING_OFFSET)),
        );
    page("Change the target while it is moving to retarget the spring. The sibling moves as width and height reflow, while paint and border geometry animate.")
        .child(row().children([
            control("Next target", cx, |s| s.spring_step = (s.spring_step + 1) % 3),
            control(if state.bouncy { "Bouncy" } else { "Critically damped" }, cx, |s| s.bouncy = !s.bouncy),
        ]))
        .child(card("Reflow and retargeting").child(row().children([
            moving,
            label("Sibling", 15., INK).size(90., 50.).padding(10.)
                .background(0xd5e8ee).corner_radius(6.),
        ])))
}
