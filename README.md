# octlui

A retained-state, GPU-rendered UI library for Rust with a [gpui](https://gpui.rs)-inspired API:
declarative node trees returned from a view function, actions bound to keys through key
contexts, and typed listeners. Pure Rust: `winit 0.31` for windowing, `wgpu` for rendering.

![showcase](docs/showcase.png)

## A taste

```rust
use octlui::{style::*, *};

struct State { value: i32 }

#[octlui::action]
struct Reset;

#[octlui::action]
struct Change { amount: i32 }

fn reset(state: &mut State, cx: &mut Context<State>, _: Reset) {
    state.value = 0;
    cx.notify();
}
fn change(state: &mut State, cx: &mut Context<State>, action: &Change) {
    state.value += action.amount;
    cx.notify();
}

fn view(state: &mut State, cx: &mut Context<State>) -> Node {
    node()
        .key_context("counter")
        .on_action(cx.listener(reset))     // handler may take the action by value...
        .on_action(cx.listener(change))    // ...or by reference
        .child(node().size(fixed(64.0), fixed(64.0)).background(0xa6e3a1)
            .on_click_action(Change { amount: 1 }))
}

fn main() {
    application().run(|acx: &mut AppContext| {
        acx.bind_keys([KeyBinding::new(Key::R, Reset, Some("counter"))]);
        acx.open_window(WindowOptions::new(), State { value: 0 }, view);
    });
}
```

## Examples

```sh
cargo run -p octlui-examples
cargo run -p octlui --example counter
cargo run -p octlui --example showcase
```

The examples crate opens a feature gallery and an independent counter window. Scroll the
sidebar or drag its thumb to browse pages. Press Esc to close the focused window.
Text shaping, wrapping, fonts and alignment have separate pages, as do floating overlays,
tooltips, image loading, image fit and image tint. The Motion section has a page for each
spring property.

To add a gallery page, put a Rust file in `crates/octlui-examples/src/features` with
`CATEGORY`, `TITLE`, and `view(state: &mut Gallery, cx: &mut Cx) -> Node`. The build script
uses its filename as the page ID. Set `OCTLUI_EXAMPLES_FONT` to a local font file to use it in
the text demo.

## Concepts

**Views and state.** `acx.open_window(options, state, view)` owns your state `S`. `view` runs
again whenever a handler calls `cx.notify()` (or the window changes); between those it is not
called, and animations/scrolling run without it.

**Listeners.** `cx.listener(f)` turns `fn(&mut S, &mut Context<S>, &E)` *or*
`fn(&mut S, &mut Context<S>, E)` into a handler; `E` is inferred. Node handlers: `on_click`,
`on_mouse_down`, `on_hover`, `on_key_down`, `on_text_change`, `on_file_drop`, `on_action`, plus
`on_click_action(action)` and `on_file_drop_action(map)`. `Context` also offers `dispatch`,
`focus`, `blur`, `set_title`, `close_window`, `quit`, `is_hovered`, and `is_focused`.

**Actions.** `#[octlui::action]` on a struct/enum (adds `Clone + Debug` if missing). Actions
dispatch from the focused node (or the root) up to the root; the first node with a matching
`on_action` handles it. Call `cx.propagate_action()` in a handler to continue up the tree.
Mapped file-drop actions start at the drop target. Unhandled actions go to
`acx.on_action::<A>(..)` global handlers.

**Application errors.** Use `application().try_run(setup)` to receive an event-loop or
window-creation error.

**Key bindings.** `KeyBinding::new(Key::S.with(Modifiers::primary()), Save, Some("editor && !readonly"))`.
Contexts are the `key_context` names along the focus path; deeper matches win, later bindings
win ties. `Keystroke::parse("ctrl-shift-s")` is available too.

**Focus.** `.focusable(true)` nodes take focus when clicked (or via `cx.focus(id)`).

## Node

Always present: `width`/`height` (`fit()`, `grow()`, `fixed(x)`, `fraction(f)` each with
`.min(..)`/`.max(..)`), `child_align_x`/`child_align_y` (`Start|Center|End`), `direction`
(`TopToBottom|BottomToTop|LeftToRight|RightToLeft`), `child_gap`.

Optional: `id`, `padding`, `corner_radius` (per corner), `aspect_ratio`, `clip`
(`None|X|Y|XY`), `key_context`, `focusable`, `on_file_drop`.

Optional components (each boxed, so plain nodes stay small):

| component | what it does |
|---|---|
| `Appearance` | background + border (width, color); colors may be gradients (`Fill::linear`, `Fill::radial`, up to 8 stops) |
| `Text` | content, color, size, font, alignment, wrapping, optional `TextInput` |
| `Image` | `ImageId` source, tint (gradient ok), fit `Contain / Cover / Fill / None`; drawn above the background, below text |
| `Tooltip` | a `Node` shown after a hover delay, with fade in/out |
| `Scroll` | child offset, smooth scrolling, wheel/track pad with unused deltas passed to ancestor scroll views, optional `ScrollBar` with a draggable thumb |
| `Floating` | lifts the node out of the flow, attached to its parent via two anchors + offset, optional clip to parent, z-index |
| `Spring` | animate position, visual size, paint, border width, corner radius, or floating offset. Use `WIDTH`, `HEIGHT`, `PADDING`, or `CHILD_GAP` to animate layout |

Combine spring properties with `|`. `SpringProps::LAYOUT` selects all four reflow
properties. Paint springs interpolate solid colors and gradient stops, and they resume from
the visible fill when retargeted. A linear-to-radial transition passes through a solid color.

### Builder macros

`Node` and every component are declared with the same macros you can use for your own types:

```rust
#[derive(Default)]
#[octlui::builder]              // generates `Self::counter_config()`
pub struct CounterConfig {
    #[octlui::property]
    step: i32,                  // pub fn step(self, i32) -> Self
    #[octlui::property]
    limit: Option<i32>,         // pub fn limit(self, i32) -> Self   (stores Some)
    #[octlui::property(into)]
    label: String,              // pub fn label(self, impl Into<String>) -> Self
}
```

Options: `#[builder(free)]` also emits a free function (`node()`), `#[builder(name = "new")]`
renames the constructor, `#[builder(no_constructor)]` skips it. Property options: `into`,
`boxed` (for `Option<Box<T>>`, setter takes `T`), `name = "..."`.

## Architecture

```
view() -> Node tree
  └─ Arena::build        BFS-flattened, children contiguous, parents before children
      └─ layout          fit W → grow/shrink W → fit H (text wraps) → grow/shrink H → position
          └─ tick        scroll smoothing, scroll-bar fades, springs, tooltip timers
              └─ paint   RenderList (quads, images, text runs) + hit regions + scroll-bar hits
                  └─ Renderer (wgpu)   one instanced draw per image batch
```

* Layout passes are plain loops over a contiguous arena (reverse = bottom-up, forward =
  top-down). Scroll-only frames update positions without remeasuring text or sizes.
* One shader draws everything: SDF rounded rectangles with per-corner radii, anti-aliased
  borders, gradient fills (storage buffer), image sampling with tint, and a **per-instance clip
  rect** (so clipping never breaks batches). Consecutive solid quads are a single draw call.
* Retained per-node state (scroll offsets, springs, fades, tooltip timers) is keyed by `Id`
  (or by tree position when no id is given). Give list items stable ids.
* `POSITION` translates the whole subtree. `SIZE` changes the node's own visual bounds.
  Layout springs update geometry and hit regions as children and siblings reflow.
* Waiting for a tooltip uses a timer. Rendering resumes when its fade begins.
* Coordinates are logical pixels; the renderer applies the window scale factor. Colors are
  sRGB values blended in gamma space (a non-sRGB surface format is preferred).
* `headless::HeadlessRenderer` renders to RGBA offscreen for screenshot tests.

## Text

Windowed and headless renderers use the system sans-serif font by default. Use
`components::FontId::SERIF` or `MONOSPACE` to select another system family.
In the application setup callback, load a local font or look up an installed font
by family or PostScript name. Use `load_font_bytes(bytes, face_index)` for in-memory font data.
Pass the returned ID to `Text::font` to select it for
one node, or set the default before opening a window:

```rust,ignore
let body = acx.load_font("assets/Body.otf").expect("load body font");
acx.set_default_font(body);
let heading = acx.load_system_font("Georgia").unwrap_or(body);
let title = octlui::components::Text::new("Settings").font(heading);
```

Layout uses the selected face's advances and line metrics in logical pixels.
The renderer rasterizes glyphs at the window's physical scale through DirectWrite
on Windows, Core Graphics on macOS, and FreeType on Linux. Coverage masks stay in
GPU atlas pages across frames. Consecutive glyphs on one page share an instanced draw.

Call `acx.enable_text_shaping()` before opening a window to enable complex-script shaping,
bidirectional layout, system font fallback, and color emoji where suitable fonts are installed.
The shaping backend caches layout by text and width and uses the same instanced glyph atlas
renderer. Headless rendering exposes the same opt-in method. Custom integrations can use
`AppContext::set_text_measurer` and `AppContext::set_text_renderer`.

### Editable text

```rust
use octlui::{node, components::{Text, TextInput}};

let search = node()
    .id("search")
    .size(260.0, 36.0)
    .padding(8.0)
    .background(0x28283a)
    .text(Text::new("").input(
        TextInput::new()
            .prompt("Search")
            .caret_color(0x93c5fd)
            .enabled(true),
    ));
```

The field retains its value and editing history across view rebuilds. Use a stable ID to
preserve them when the field moves among siblings. Changing the declared `Text::content`
replaces the value. Attach `Node::on_text_change` to receive a `TextChangeEvent` with the new
value after edits, undo, and redo. Call `cx.notify()` from the listener if other parts of the
view depend on that value.

Inputs are single-line by default. Use `multiline(true)` for line breaks and word or letter
wrapping, or `enabled(false)` for a read-only field with selection and copying. Inputs always
use the built-in Unicode shaping backend so the glyphs, caret, and selection share a layout.

Click or Tab to focus, drag to select, double-click for a word, or triple-click for a
paragraph. The editor handles character, word, line, paragraph, document, and page
navigation, Shift selection, clipboard commands, and undo/redo. It uses Ctrl-based
shortcuts on Windows and Linux, and Command, Option, and Control text bindings on macOS.
Unclaimed keys continue through the app's key bindings and `on_key_down` handlers.
Single-line fields leave Enter available for submission handlers.

## Images

`acx.images().insert_rgba(w, h, pixels)` inserts raw pixels. To decode PNG or JPEG on demand,
call `acx.register_image(bytes)` or `acx.register_image_supplier(|| std::fs::read(path))`.
The supplier and decoder run on a worker. Once pixels arrive, the runtime requests layout and
redraw, including for images whose dimensions determine their size. A synchronous
`ImageSupplier` remains available for raw pixels loaded by ID.

## Platform notes

* File drops use winit 0.31's data-transfer API (`UriList`); `FileDropEvent` carries paths.
* Requires a Rust toolchain recent enough for `winit 0.31.0-beta.4` and `wgpu 30`.
* Tests: `cargo test` (GPU tests skip if no adapter is found; CI uses lavapipe).

## License

MIT OR Apache-2.0
