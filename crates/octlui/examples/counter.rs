//! The counter from the design sketch: actions, key bindings, listeners.
//! `cargo run --example counter` — press R to reset, Up/Down to change, click the buttons.
//! The counter displays its value above the bar.

use octlui::geometry::*;
use octlui::style::*;
use octlui::*;

const MAX_COUNT: i32 = 50;

struct State {
    value: i32,
}

#[octlui::action]
struct Reset;

#[octlui::action]
struct Change {
    amount: i32,
}

fn reset(state: &mut State, cx: &mut Context<State>, _: Reset) {
    state.value = 0;
    cx.notify();
}

fn change(state: &mut State, cx: &mut Context<State>, action: &Change) {
    state.value = (state.value + action.amount).clamp(0, MAX_COUNT);
    cx.notify();
}

fn on_click_increment(state: &mut State, cx: &mut Context<State>, _: &events::ClickEvent) {
    state.value = (state.value + 1).clamp(0, MAX_COUNT);
    cx.notify();
}

fn view(state: &mut State, cx: &mut Context<State>) -> Node {
    let button = |label: &str, color: u32, ink: u32| {
        node()
            .size(fixed(80.0), fixed(48.0))
            .corner_radius(16.0)
            .background(color)
            .center()
            .child(node().text(components::Text::new(label).color(ink)))
            .spring(components::Spring::new(components::SpringProps::BACKGROUND))
    };
    node()
        .key_context("counter")
        .on_action(cx.listener(reset))
        .on_action(cx.listener(change))
        .size(grow(), grow())
        .column()
        .center()
        .child_gap(24.0)
        .child(node().text(components::Text::new(format!("{}/{MAX_COUNT}", state.value)).size(24.0)))
        .child(
            node()
                .size(fixed(400.0), fixed(24.0))
                .corner_radius(12.0)
                .background(0x313244)
                .clip(Clip::X)
                .child(
                    node()
                        .size(fraction(state.value as f32 / MAX_COUNT as f32), grow())
                        .spring(components::Spring::new(components::SpringProps::SIZE))
                        .corner_radius(12.0)
                        .background(Fill::linear(0.0, &[Color::hex(0x89b4fa), Color::hex(0xcba6f7)])),
                ),
        )
        .child(
            node()
                .row()
                .child_gap(16.0)
                .child(button("-1", 0xf38ba8, 0x181825).on_click_action(Change { amount: -1 }))
                .child(button("+1", 0xa6e3a1, 0x181825).on_click(cx.listener(on_click_increment)))
                .child(button("Reset", 0x585b70, 0xffffff).on_click_action(Reset)),
        )
        .child(node().text(components::Text::new("Up / Down to change · R to reset").size(14.0)))
}

fn main() {
    application().run(|acx: &mut AppContext| {
        acx.bind_keys([
            KeyBinding::new(Key::R, Reset, Some("counter")),
            KeyBinding::new(Key::Up, Change { amount: 1 }, Some("counter")),
            KeyBinding::new(Key::Down, Change { amount: -1 }, Some("counter")),
            KeyBinding::new(Key::Q.with(Modifiers::primary()), Quit, None),
        ]);
        acx.on_action(|_: &Quit, acx| acx.quit());
        acx.open_window(WindowOptions::new().title("octlui counter"), State { value: 5 }, view);
    });
}

#[octlui::action]
struct Quit;
