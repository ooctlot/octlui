//! Application entry point, windows and the winit event loop.

use crate::action::{Action, Event, Key, Keymap, KeyBinding, Modifiers};
use crate::ctx::{Context, Effect, WindowInfo};
use crate::events::MouseButton;
use crate::geometry::{Color, Vec2};
use crate::image::ImageStore;
use crate::layout::LayoutEnv;
use crate::node::Node;
use crate::render::{GpuContext, NoopTextRenderer, Renderer, TextRenderer};
use crate::text::{PlaceholderMeasurer, TextMeasurer};
use crate::ui::{Env, Ui};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::data_transfer::TypeHint;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, DndAction, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window as WinitWindow, WindowAttributes, WindowId};

/// Logical pixels scrolled per mouse wheel "line".
const LINE_SCROLL_PX: f32 = 48.0;

/// Options for a new window.
#[derive(Clone)]
#[octlui::builder(name = "new")]
pub struct WindowOptions {
    #[octlui::property(into)]
    pub title: String,
    /// Initial size in logical pixels.
    #[octlui::property]
    pub size: Vec2,
    #[octlui::property]
    pub min_size: Option<Vec2>,
    #[octlui::property]
    pub resizable: bool,
    /// Color the window is cleared with every frame.
    #[octlui::property(into)]
    pub clear_color: Color,
    #[octlui::property]
    pub present_mode: wgpu::PresentMode,
}
impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "octlui".into(),
            size: Vec2::new(960.0, 640.0),
            min_size: None,
            resizable: true,
            clear_color: Color::hex(0x1e1e2e),
            present_mode: wgpu::PresentMode::AutoVsync,
        }
    }
}

type GlobalHandler = Rc<dyn Fn(&dyn Any, &mut AppContext)>;
type TextMeasurerFactory = Box<dyn Fn() -> Box<dyn TextMeasurer>>;
type TextRendererFactory = Box<dyn Fn() -> Box<dyn TextRenderer>>;

/// Application-wide state handed to the `run` closure and to global action handlers.
pub struct AppContext {
    pub(crate) keymap: Keymap,
    pub(crate) images: ImageStore,
    pending: Vec<Box<dyn PendingWindow>>,
    globals: Vec<(TypeId, GlobalHandler)>,
    quit: bool,
    measurer: TextMeasurerFactory,
    text_renderer: TextRendererFactory,
}

impl AppContext {
    fn new() -> Self {
        Self {
            keymap: Keymap::default(),
            images: ImageStore::default(),
            pending: Vec::new(),
            globals: Vec::new(),
            quit: false,
            measurer: Box::new(|| Box::new(PlaceholderMeasurer)),
            text_renderer: Box::new(|| Box::new(NoopTextRenderer)),
        }
    }

    /// Registers key bindings.
    pub fn bind_keys(&mut self, bindings: impl IntoIterator<Item = KeyBinding>) {
        for b in bindings {
            self.keymap.add(b);
        }
    }

    /// Handles action `A` when no node on the focus path did.
    pub fn on_action<A: Action + Event>(&mut self, handler: impl Fn(&A, &mut AppContext) + 'static) {
        self.globals.push((
            TypeId::of::<A>(),
            Rc::new(move |a, acx| {
                if let Some(a) = a.downcast_ref::<A>() {
                    handler(a, acx)
                }
            }),
        ));
    }

    /// Images shared by all windows.
    pub fn images(&mut self) -> &mut ImageStore {
        &mut self.images
    }

    /// Replaces the text measurer used by new windows (real text shaping plugs in here).
    pub fn set_text_measurer(&mut self, f: impl Fn() -> Box<dyn TextMeasurer> + 'static) {
        self.measurer = Box::new(f);
    }
    /// Replaces the text renderer used by new windows.
    pub fn set_text_renderer(&mut self, f: impl Fn() -> Box<dyn TextRenderer> + 'static) {
        self.text_renderer = Box::new(f);
    }

    /// Quits the application after the current event.
    pub fn quit(&mut self) {
        self.quit = true;
    }

    /// Opens a window showing `view(&mut state, cx)`.
    pub fn open_window<S: 'static>(
        &mut self,
        options: WindowOptions,
        state: S,
        view: impl Fn(&mut S, &mut Context<S>) -> Node + 'static,
    ) {
        self.pending.push(Box::new(WindowSpec { options, state, view: Box::new(view) }));
    }

    fn route_global(&mut self, a: Box<dyn Action>) {
        let tid = a.type_id_of();
        let handlers: Vec<GlobalHandler> =
            self.globals.iter().filter(|(t, _)| *t == tid).map(|(_, h)| h.clone()).collect();
        for h in handlers {
            h(a.as_any(), self);
        }
    }
}

// ---------------------------------------------------------------------------------------------

trait PendingWindow {
    fn create(
        self: Box<Self>,
        el: &dyn ActiveEventLoop,
        gpu: &mut Option<Rc<GpuContext>>,
        app: &AppContext,
    ) -> Result<Box<dyn UiWindow>, Box<dyn std::error::Error>>;
}

trait UiWindow {
    fn id(&self) -> WindowId;
    /// Returns `false` when the window wants to close.
    fn event(&mut self, app: &mut AppContext, el: &dyn ActiveEventLoop, ev: WindowEvent) -> bool;
}

struct WindowSpec<S> {
    options: WindowOptions,
    state: S,
    view: Box<dyn Fn(&mut S, &mut Context<S>) -> Node>,
}

struct Window<S> {
    win: Arc<dyn WinitWindow>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Rc<GpuContext>,
    renderer: Renderer,
    ui: Ui,
    state: S,
    cx: Context<S>,
    view: Box<dyn Fn(&mut S, &mut Context<S>) -> Node>,
    measurer: Box<dyn TextMeasurer>,
    clear: Color,
    dirty: bool,
    last_frame: Instant,
    drag_pos: Vec2,
    drag_paths: Vec<PathBuf>,
}

impl<S: 'static> PendingWindow for WindowSpec<S> {
    fn create(
        self: Box<Self>,
        el: &dyn ActiveEventLoop,
        gpu: &mut Option<Rc<GpuContext>>,
        app: &AppContext,
    ) -> Result<Box<dyn UiWindow>, Box<dyn std::error::Error>> {
        let o = &self.options;
        let mut attrs = WindowAttributes::default()
            .with_title(o.title.clone())
            .with_surface_size(winit::dpi::LogicalSize::new(o.size.x as f64, o.size.y as f64))
            .with_resizable(o.resizable);
        if let Some(m) = o.min_size {
            attrs = attrs.with_min_surface_size(winit::dpi::LogicalSize::new(m.x as f64, m.y as f64));
        }
        let win: Arc<dyn WinitWindow> = Arc::from(el.create_window(attrs)?);

        let (gpu_ctx, surface) = match gpu {
            Some(g) => {
                let s = g.instance.create_surface(wgpu::SurfaceTarget::from_window_without_display(win.clone()))?;
                (g.clone(), s)
            }
            None => {
                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                    Box::new(el.owned_display_handle()),
                ));
                let s = instance.create_surface(wgpu::SurfaceTarget::from_window_without_display(win.clone()))?;
                let g = Rc::new(GpuContext::new(instance, Some(&s))?);
                *gpu = Some(g.clone());
                (g, s)
            }
        };

        let size = win.surface_size();
        let mut config = surface
            .get_default_config(&gpu_ctx.adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface is not supported by the adapter")?;
        let caps = surface.get_capabilities(&gpu_ctx.adapter);
        // Colors are authored in sRGB and blended in gamma space, like most UI toolkits, so
        // prefer a non-sRGB target format; the shader converts if only sRGB formats exist.
        if let Some(f) = caps.formats.iter().copied().find(|f| !f.is_srgb()) {
            config.format = f;
        }
        config.present_mode = o.present_mode;
        surface.configure(&gpu_ctx.device, &config);

        let mut renderer = Renderer::new(&gpu_ctx, config.format);
        renderer.set_text_renderer((app.text_renderer)());

        let mut ui = Ui::new();
        let scale = win.scale_factor() as f32;
        ui.set_size(Vec2::new(size.width as f32 / scale, size.height as f32 / scale), scale);

        let mut cx = Context::new();
        cx.info = WindowInfo { size: ui.size, scale, focus: None, hover: Vec::new() };
        win.request_redraw();
        Ok(Box::new(Window {
            win,
            surface,
            config,
            gpu: gpu_ctx,
            renderer,
            ui,
            state: self.state,
            cx,
            view: self.view,
            measurer: (app.measurer)(),
            clear: self.options.clear_color,
            dirty: true,
            last_frame: Instant::now(),
            drag_pos: Vec2::ZERO,
            drag_paths: Vec::new(),
        }))
    }
}

fn map_key(code: KeyCode) -> Option<Key> {
    use KeyCode as K;
    Some(match code {
        K::KeyA => Key::A, K::KeyB => Key::B, K::KeyC => Key::C, K::KeyD => Key::D, K::KeyE => Key::E,
        K::KeyF => Key::F, K::KeyG => Key::G, K::KeyH => Key::H, K::KeyI => Key::I, K::KeyJ => Key::J,
        K::KeyK => Key::K, K::KeyL => Key::L, K::KeyM => Key::M, K::KeyN => Key::N, K::KeyO => Key::O,
        K::KeyP => Key::P, K::KeyQ => Key::Q, K::KeyR => Key::R, K::KeyS => Key::S, K::KeyT => Key::T,
        K::KeyU => Key::U, K::KeyV => Key::V, K::KeyW => Key::W, K::KeyX => Key::X, K::KeyY => Key::Y,
        K::KeyZ => Key::Z,
        K::Digit0 => Key::Num0, K::Digit1 => Key::Num1, K::Digit2 => Key::Num2, K::Digit3 => Key::Num3,
        K::Digit4 => Key::Num4, K::Digit5 => Key::Num5, K::Digit6 => Key::Num6, K::Digit7 => Key::Num7,
        K::Digit8 => Key::Num8, K::Digit9 => Key::Num9,
        K::F1 => Key::F1, K::F2 => Key::F2, K::F3 => Key::F3, K::F4 => Key::F4, K::F5 => Key::F5,
        K::F6 => Key::F6, K::F7 => Key::F7, K::F8 => Key::F8, K::F9 => Key::F9, K::F10 => Key::F10,
        K::F11 => Key::F11, K::F12 => Key::F12,
        K::Enter | K::NumpadEnter => Key::Enter,
        K::Escape => Key::Escape, K::Space => Key::Space, K::Tab => Key::Tab,
        K::Backspace => Key::Backspace, K::Delete => Key::Delete, K::Insert => Key::Insert,
        K::Home => Key::Home, K::End => Key::End, K::PageUp => Key::PageUp, K::PageDown => Key::PageDown,
        K::ArrowUp => Key::Up, K::ArrowDown => Key::Down, K::ArrowLeft => Key::Left, K::ArrowRight => Key::Right,
        K::Minus => Key::Minus, K::Equal => Key::Equal, K::Comma => Key::Comma, K::Period => Key::Period,
        K::Slash => Key::Slash, K::Backslash => Key::Backslash, K::Semicolon => Key::Semicolon,
        K::Quote => Key::Quote, K::Backquote => Key::Backquote,
        K::BracketLeft => Key::BracketLeft, K::BracketRight => Key::BracketRight,
        _ => return None,
    })
}

fn map_button(b: winit::event::MouseButton) -> MouseButton {
    use winit::event::MouseButton as B;
    match b {
        B::Left => MouseButton::Left,
        B::Right => MouseButton::Right,
        B::Middle => MouseButton::Middle,
        other => MouseButton::Other(other as u16),
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 0 && i + 2 <= b.len() - 1 {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Converts a `file://` URI into a path.
pub(crate) fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // Drop an optional authority ("localhost").
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let decoded = percent_decode(rest);
    // "/C:/dir" on Windows.
    let bytes = decoded.as_bytes();
    if bytes.len() > 2 && bytes[0] == b'/' && bytes[2] == b':' && bytes[1].is_ascii_alphabetic() {
        return Some(PathBuf::from(&decoded[1..]));
    }
    Some(PathBuf::from(decoded))
}

impl<S: 'static> Window<S> {
    fn sync_info(&mut self) {
        self.cx.info = WindowInfo {
            size: self.ui.size,
            scale: self.ui.scale,
            focus: self.ui.focus,
            hover: self.ui.hover_keys(),
        };
    }

    /// Applies effects requested by handlers until none are left.
    fn pump(&mut self, app: &mut AppContext) {
        for _ in 0..64 {
            let effects = std::mem::take(&mut self.cx.effects);
            let unhandled = std::mem::take(&mut self.ui.unhandled);
            if effects.is_empty() && unhandled.is_empty() {
                break;
            }
            for a in unhandled {
                app.route_global(a);
            }
            for e in effects {
                match e {
                    Effect::Notify => self.dirty = true,
                    Effect::Quit => app.quit = true,
                    Effect::Focus(id) => {
                        self.ui.set_focus(Some(&id));
                        self.dirty = true;
                    }
                    Effect::Blur => {
                        self.ui.set_focus(None);
                        self.dirty = true;
                    }
                    Effect::SetTitle(t) => self.win.set_title(&t),
                    Effect::Dispatch(a) => {
                        self.sync_info();
                        let handled = {
                            let mut env = Env { state: &mut self.state, cx: &mut self.cx };
                            self.ui.dispatch_action(&*a, &mut env)
                        };
                        if !handled {
                            app.route_global(a);
                        }
                    }
                }
            }
        }
        if self.dirty {
            self.win.request_redraw();
        }
    }

    fn logical(&self, p: winit::dpi::PhysicalPosition<f64>) -> Vec2 {
        let s = self.ui.scale as f64;
        Vec2::new((p.x / s) as f32, (p.y / s) as f32)
    }

    fn resize(&mut self) {
        let size = self.win.surface_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.gpu.device, &self.config);
        let scale = self.win.scale_factor() as f32;
        self.ui.set_size(Vec2::new(size.width as f32 / scale, size.height as f32 / scale), scale);
        self.win.request_redraw();
    }

    fn redraw(&mut self, app: &mut AppContext) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;

        let mut rebuilt = false;
        if self.dirty || !self.ui.has_root() {
            self.dirty = false;
            self.sync_info();
            let node = (self.view)(&mut self.state, &mut self.cx);
            self.ui.set_root(node);
            rebuilt = true;
            self.pump(app);
        }

        let animating = {
            let mut lenv = LayoutEnv { text: &mut *self.measurer, images: &mut app.images };
            self.ui.frame(dt, &mut lenv)
        };

        // Re-evaluate hover against the fresh layout (content may have moved under the pointer).
        if rebuilt || animating {
            self.sync_info();
            let mut env = Env { state: &mut self.state, cx: &mut self.cx };
            self.ui.pointer_moved(None, &mut env);
            self.pump(app);
        }

        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                self.renderer.render(
                    &self.gpu,
                    &view,
                    (self.config.width, self.config.height),
                    self.ui.scale,
                    &self.ui.list,
                    self.clear,
                    &mut app.images,
                );
                self.win.pre_present_notify();
                self.gpu.queue.present(frame);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                self.win.request_redraw();
            }
            _ => {}
        }

        if animating || self.dirty {
            self.win.request_redraw();
        }
    }
}

impl<S: 'static> UiWindow for Window<S> {
    fn id(&self) -> WindowId {
        self.win.id()
    }

    fn event(&mut self, app: &mut AppContext, el: &dyn ActiveEventLoop, ev: WindowEvent) -> bool {
        match ev {
            WindowEvent::CloseRequested => return false,
            WindowEvent::SurfaceResized(_) => self.resize(),
            WindowEvent::ScaleFactorChanged { .. } => self.resize(),
            WindowEvent::RedrawRequested => self.redraw(app),
            WindowEvent::Occluded(false) => self.win.request_redraw(),

            WindowEvent::PointerMoved { position, primary: true, .. } => {
                let p = self.logical(position);
                self.sync_info();
                let changed = {
                    let mut env = Env { state: &mut self.state, cx: &mut self.cx };
                    self.ui.pointer_moved(Some(p), &mut env)
                };
                if changed {
                    self.win.request_redraw();
                }
                self.pump(app);
            }
            WindowEvent::PointerLeft { primary: true, .. } => {
                self.sync_info();
                {
                    let mut env = Env { state: &mut self.state, cx: &mut self.cx };
                    self.ui.pointer_left(&mut env);
                }
                self.win.request_redraw();
                self.pump(app);
            }
            WindowEvent::PointerButton { state, position, primary: true, button, .. } => {
                if let Some(b) = button.mouse_button() {
                    let p = self.logical(position);
                    self.sync_info();
                    {
                        let mut env = Env { state: &mut self.state, cx: &mut self.cx };
                        self.ui.pointer_button(map_button(b), state == ElementState::Pressed, Some(p), &mut env);
                    }
                    self.win.request_redraw();
                    self.pump(app);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(x, y) => Vec2::new(x * LINE_SCROLL_PX, y * LINE_SCROLL_PX),
                    MouseScrollDelta::PixelDelta(p) => self.logical(p),
                    _ => Vec2::ZERO,
                };
                if self.ui.wheel(d) {
                    self.win.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                let mut mods = Modifiers::NONE;
                if s.shift_key() {
                    mods = mods | Modifiers::SHIFT;
                }
                if s.control_key() {
                    mods = mods | Modifiers::CTRL;
                }
                if s.alt_key() {
                    mods = mods | Modifiers::ALT;
                }
                if s.meta_key() {
                    mods = mods | Modifiers::META;
                }
                self.ui.mods = mods;
            }
            WindowEvent::Focused(false) => self.ui.mods = Modifiers::NONE,
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let key = match event.physical_key {
                    PhysicalKey::Code(c) => map_key(c),
                    _ => None,
                };
                let text: Option<Arc<str>> = event.text.as_ref().map(|t| Arc::from(t.as_str()));
                self.sync_info();
                {
                    let mut env = Env { state: &mut self.state, cx: &mut self.cx };
                    self.ui.key_down(key, text, event.repeat, &app.keymap, &mut env);
                }
                self.pump(app);
            }

            // ---- file drag & drop ----
            WindowEvent::DragEntered { id, position } => {
                if let Some(p) = position {
                    self.drag_pos = self.logical(p);
                }
                self.drag_paths.clear();
                let wants = el.data_transfer(id).map(|t| t.has_type(&TypeHint::UriList)).unwrap_or(false);
                if wants {
                    let _ = el.set_valid_dnd_actions(id, &[DndAction::Copy]);
                    let _ = el.fetch_data_transfer(id, &TypeHint::UriList);
                } else {
                    let _ = el.set_valid_dnd_actions(id, &[]);
                }
            }
            WindowEvent::DragPosition { position, .. } => self.drag_pos = self.logical(position),
            WindowEvent::DataTransferReceived { value, .. } => {
                if matches!(value.type_().hint(), Some(TypeHint::UriList)) {
                    if let Ok(uris) = value.try_as_uris() {
                        self.drag_paths = uris.iter().filter_map(|u| uri_to_path(u)).collect();
                    }
                }
            }
            WindowEvent::DragDropped { .. } => {
                let paths = std::mem::take(&mut self.drag_paths);
                if !paths.is_empty() {
                    self.sync_info();
                    {
                        let mut env = Env { state: &mut self.state, cx: &mut self.cx };
                        self.ui.file_drop(paths, self.drag_pos, &mut env);
                    }
                    self.pump(app);
                }
            }
            WindowEvent::DragLeft { .. } => self.drag_paths.clear(),
            _ => {}
        }
        true
    }
}

// ---------------------------------------------------------------------------------------------

struct Runner {
    init: Option<Box<dyn FnOnce(&mut AppContext)>>,
    app: AppContext,
    gpu: Option<Rc<GpuContext>>,
    windows: HashMap<WindowId, Box<dyn UiWindow>>,
    failed: bool,
}

impl Runner {
    fn open_pending(&mut self, el: &dyn ActiveEventLoop) {
        let pending = std::mem::take(&mut self.app.pending);
        for p in pending {
            match p.create(el, &mut self.gpu, &self.app) {
                Ok(w) => {
                    self.windows.insert(w.id(), w);
                }
                Err(e) => {
                    eprintln!("octlui: failed to open window: {e}");
                    self.failed = true;
                }
            }
        }
    }
}

impl ApplicationHandler for Runner {
    fn can_create_surfaces(&mut self, el: &dyn ActiveEventLoop) {
        if let Some(init) = self.init.take() {
            init(&mut self.app);
        }
        self.open_pending(el);
        if self.windows.is_empty() {
            el.exit();
        }
    }

    fn window_event(&mut self, el: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let keep = match self.windows.get_mut(&id) {
            Some(w) => w.event(&mut self.app, el, event),
            None => true,
        };
        if !keep {
            self.windows.remove(&id);
        }
        self.open_pending(el);
        if self.app.quit || self.windows.is_empty() || self.failed && self.windows.is_empty() {
            el.exit();
        }
    }
}

/// Builder for the application. See [`application`].
pub struct Application {
    _private: (),
}

/// Starts building an application.
///
/// ```ignore
/// application().run(|acx| {
///     acx.bind_keys([KeyBinding::new(Key::R, Reset, Some("counter"))]);
///     acx.open_window(WindowOptions::new(), State::default(), view);
/// });
/// ```
pub fn application() -> Application {
    Application { _private: () }
}

impl Application {
    /// Runs the event loop. `init` is called once the platform is ready; open windows and
    /// register key bindings there. Returns when the last window closes or the app quits.
    pub fn run(self, init: impl FnOnce(&mut AppContext) + 'static) {
        let event_loop = match EventLoop::new() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("octlui: failed to create the event loop: {e}");
                return;
            }
        };
        let runner = Runner {
            init: Some(Box::new(init)),
            app: AppContext::new(),
            gpu: None,
            windows: HashMap::new(),
            failed: false,
        };
        if let Err(e) = event_loop.run_app(runner) {
            eprintln!("octlui: event loop error: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uris() {
        assert_eq!(uri_to_path("file:///home/a%20b/c.png"), Some(PathBuf::from("/home/a b/c.png")));
        assert_eq!(uri_to_path("file:///C:/x/y.txt"), Some(PathBuf::from("C:/x/y.txt")));
        assert_eq!(uri_to_path("http://x"), None);
    }
}
