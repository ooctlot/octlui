use octlui::{components::*, geometry::Vec2, image::ImageId, style::*, *};
use std::path::PathBuf;
use ui::*;

mod spring_demo;
mod ui;

type Cx = Context<Gallery>;

struct Feature {
    id: &'static str,
    category: &'static str,
    title: &'static str,
    view: fn(&mut Gallery, &mut Cx) -> Node,
}

include!(concat!(env!("OUT_DIR"), "/features.rs"));

const CATEGORIES: &[&str] = &[
    "Layout",
    "Rendering",
    "Text",
    "Interaction",
    "Motion",
    "Application",
];

#[action]
struct Select(&'static str);
#[action]
struct Close;
#[action]
struct Add(i32);
#[action]
struct Reset;
#[action]
struct Dropped(Vec<PathBuf>);

#[derive(Default)]
struct Gallery {
    selected: &'static str,
    direction: usize,
    align: usize,
    clip: usize,
    overlay_clipped: bool,
    scroll_offset: usize,
    no_smooth: bool,
    spring_step: usize,
    bouncy: bool,
    tooltip_slow: bool,
    hovering: bool,
    presses: usize,
    last_key: Option<Key>,
    child_actions: usize,
    parent_actions: usize,
    count: i32,
    drop_paths: Vec<PathBuf>,
    image: Option<ImageId>,
    lazy_image: Option<ImageId>,
    loaded_font: Option<FontId>,
}

fn gallery(state: &mut Gallery, cx: &mut Cx) -> Node {
    let feature = FEATURES
        .iter()
        .find(|page| page.id == state.selected)
        .unwrap_or(&FEATURES[0]);
    let mut links = node()
        .id("gallery-sidebar")
        .column()
        .width(grow())
        .height(grow())
        .child_gap(14.)
        .scroll(Scroll::scroll().scroll_bar(bar().always_visible(false).reserve_space(true)));
    for &category in CATEGORIES {
        let pages = FEATURES.iter().filter(|page| page.category == category);
        let group = node()
            .column()
            .width(grow())
            .child_gap(3.)
            .child(label(category, 12., MUTED).padding((10., 4.)))
            .children(pages.map(|page| {
                node()
                    .id(page.id)
                    .width(grow())
                    .padding((10., 7.))
                    .corner_radius(6.)
                    .background(if feature.id == page.id { 0x245353 } else { NAV })
                    .text(Text::new(page.title).size(14.).color(0xe4eded))
                    .on_click_action(Select(page.id))
            }));
        links = links.child(group);
    }
    let sidebar = node()
        .column()
        .size(224., grow())
        .padding(16.)
        .child_gap(16.)
        .background(NAV)
        .child(label("octlui", 27., 0xf2f7f7))
        .child(links);
    let header = node()
        .column()
        .width(grow())
        .padding((26., 17.))
        .child_gap(4.)
        .background(CARD)
        .child(label(feature.category, 13., ACCENT))
        .child(label(feature.title, 29., INK));
    let body = node()
        .column()
        .id((
            "page",
            FEATURES.iter().position(|p| p.id == feature.id).unwrap(),
        ))
        .fill_parent()
        .padding(26.)
        .scroll(Scroll::scroll().scroll_bar(bar()))
        .child((feature.view)(state, cx));
    node()
        .fill_parent()
        .row()
        .background(PAPER)
        .on_action(
            cx.listener(|state: &mut Gallery, cx: &mut Cx, select: &Select| {
                state.selected = select.0;
                cx.notify();
            }),
        )
        .on_action(cx.listener(|_: &mut Gallery, cx: &mut Cx, _: &Close| cx.close_window()))
        .children([
            sidebar,
            node().column().fill_parent().children([header, body]),
        ])
}

fn counter(count: &mut u32, cx: &mut Context<u32>) -> Node {
    use octlui::events::ClickEvent;
    node()
        .column()
        .fill_parent()
        .padding(24.)
        .child_gap(12.)
        .background(PAPER)
        .on_action(cx.listener(|_: &mut u32, cx: &mut Context<u32>, _: Close| cx.close_window()))
        .child(label("Independent counter", 24., INK))
        .child(note(
            "This window owns its own state. Press Esc to close it.",
        ))
        .child(button(format!("Count {count}")).on_click(cx.listener(
            |count: &mut u32, cx: &mut Context<u32>, _: &ClickEvent| {
                *count = count.saturating_add(1);
                cx.notify();
            },
        )))
}

fn main() -> Result<(), RunError> {
    application().try_run(|app| {
        app.enable_text_shaping();
        app.bind_keys([
            KeyBinding::new(Key::Escape, Close, None),
            KeyBinding::new(Key::R, Reset, Some("gallery-counter")),
            KeyBinding::new(
                Key::P.with(Modifiers::primary()),
                Add(10),
                Some("gallery-counter"),
            ),
        ]);
        let image = demo_png(false).and_then(|bytes| app.register_image(bytes));
        let lazy_image = app.register_image_supplier(|| demo_png(true));
        let report = |result: std::io::Result<ImageId>| {
            result
                .map_err(|e| {
                    eprintln!("gallery image registration: {e}");
                })
                .ok()
        };
        let loaded_font = std::env::var_os("OCTLUI_EXAMPLES_FONT")
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| app.load_font_bytes(bytes, 0).ok())
            .or_else(|| {
                ["Georgia", "DejaVu Serif", "Times New Roman"]
                    .iter()
                    .find_map(|name| app.load_system_font(name).ok())
            });
        app.open_window(
            WindowOptions::new()
                .title("octlui examples")
                .size(Vec2::new(1080., 750.))
                .min_size(Vec2::new(760., 530.))
                .clear_color(PAPER),
            Gallery {
                selected: "dimensions",
                image: report(image),
                lazy_image: report(lazy_image),
                loaded_font,
                ..Default::default()
            },
            gallery,
        );
        app.open_window(
            WindowOptions::new()
                .title("octlui examples · counter")
                .size(Vec2::new(380., 205.))
                .clear_color(PAPER),
            0u32,
            counter,
        );
    })
}
