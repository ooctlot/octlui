//! Interactive showcase of the node properties. Run with `cargo run --example showcase`.
//! (Text is not rendered yet, so labels are represented by colored blocks.)

use octlui::components::*;
use octlui::geometry::*;
use octlui::style::*;
use octlui::*;
use std::time::Duration;

pub fn showcase() -> Node {
    let card = |c: u32| node().size(fixed(56.0), fixed(56.0)).corner_radius(12.0).background(c);

    // Sidebar with a scrollable list.
    let list_items = (0..24).map(|i| {
        let shade = 0x2a2a40 + ((i % 2) * 0x0a0a0a);
        node()
            .width(grow())
            .height(fixed(36.0))
            .corner_radius(8.0)
            .background(shade as u32)
            .padding((10.0, 0.0))
            .align(Align::Start, Align::Center)
            .child(node().size(fixed(60.0 + (i * 13 % 90) as f32), fixed(8.0)).corner_radius(4.0).background(0x6c7086))
    });
    let sidebar = node()
        .id("sidebar")
        .size(fixed(240.0), grow())
        .column()
        .child_gap(6.0)
        .padding(10.0)
        .corner_radius(16.0)
        .background(0x181825)
        .border(0x313244, 1.0)
        .scroll(
            Scroll::scroll()
                .scroll_bar(ScrollBar::scroll_bar().thickness(6.0).corner_radius(3.0).fade_out(Duration::from_millis(400))),
        )
        .children(list_items);

    // Gradients.
    let gradients = node()
        .row()
        .child_gap(14.0)
        .child(
            node()
                .size(fixed(120.0), fixed(80.0))
                .corner_radius([24.0, 4.0, 24.0, 4.0])
                .background(Fill::linear(0.6, &[Color::hex(0xf38ba8), Color::hex(0xfab387), Color::hex(0xf9e2af)])),
        )
        .child(
            node()
                .size(fixed(80.0), fixed(80.0))
                .corner_radius(40.0)
                .background(Fill::radial(&[Color::hex(0x89dceb), Color::hex(0x1e66f5)]))
                .border(Fill::linear(1.57, &[Color::hex(0xcba6f7), Color::hex(0x89b4fa)]), 4.0),
        )
        .child(
            node()
                .size(fixed(120.0), fixed(80.0))
                .corner_radius(10.0)
                .background(0x313244)
                .border(0xa6e3a1, 2.0),
        );

    // Springs: hover to see the card grow with a spring.
    let springy = node().row().child_gap(10.0).children((0..4).map(|i| {
        card([0xf38ba8, 0xa6e3a1, 0x89b4fa, 0xf9e2af][i])
            .id(("spring", i))
            .tooltip(Tooltip::new(
                node().padding(8.0).corner_radius(8.0).background(0x11111b).border(0x45475a, 1.0).child(
                    node().size(fixed(70.0), fixed(10.0)).corner_radius(5.0).background(0xcdd6f4),
                ),
            ))
            .spring(Spring::bouncy(SpringProps::BACKGROUND | SpringProps::SIZE))
    }));

    // Alignment & layout directions.
    let aligned = node()
        .size(fixed(300.0), fixed(100.0))
        .corner_radius(12.0)
        .background(0x181825)
        .padding(8.0)
        .child_gap(8.0)
        .align(Align::End, Align::Center)
        .direction(Direction::RightToLeft)
        .child(card(0x585b70).size(fixed(30.0), fixed(30.0)))
        .child(card(0x6c7086).size(fixed(30.0), fixed(60.0)))
        .child(card(0x7f849c).size(grow().max(80.0), fixed(20.0)));

    // Floating badge anchored to the top right of its parent.
    let with_badge = node()
        .size(fixed(90.0), fixed(90.0))
        .corner_radius(16.0)
        .background(0x45475a)
        .child(
            node()
                .size(fixed(24.0), fixed(24.0))
                .corner_radius(12.0)
                .background(0xf38ba8)
                .border(0x1e1e2e, 3.0)
                .floating(
                    Floating::floating()
                        .parent_anchor(Anchor::TOP_RIGHT)
                        .anchor(Anchor::CENTER)
                        .z_index(10),
                ),
        );

    let main = node()
        .size(grow(), grow())
        .column()
        .child_gap(20.0)
        .padding(20.0)
        .child(gradients)
        .child(springy)
        .child(node().row().child_gap(20.0).child(aligned).child(with_badge))
        .child(node().size(grow(), fixed(10.0)).corner_radius(5.0).background(0x313244).clip(Clip::X).child(
            node().width(fraction(0.6)).height(grow()).corner_radius(5.0).background(Fill::linear(
                0.0,
                &[Color::hex(0x89b4fa), Color::hex(0xcba6f7)],
            )),
        ));

    node().size(grow(), grow()).row().child_gap(16.0).padding(16.0).child(sidebar).child(main)
}

#[allow(dead_code)]
fn main() {
    struct Showcase;
    application().run(|acx: &mut AppContext| {
        acx.open_window(WindowOptions::new().title("octlui showcase"), Showcase, |_: &mut Showcase, _: &mut Context<Showcase>| showcase());
    });
}
