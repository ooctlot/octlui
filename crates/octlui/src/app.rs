//! Application entry point, windows and the winit event loop.

use crate::action::{Action, Event, Key, KeyBinding, Keymap, Modifiers};
use crate::components::FontId;
use crate::ctx::{Context, Effect, WindowInfo};
use crate::events::{FileDropEvent, MouseButton};
use crate::geometry::{Color, Vec2};
use crate::image::{ImageId, ImageStore};
use crate::layout::LayoutEnv;
use crate::node::Node;
use crate::render::{GpuContext, Renderer, TextRenderer};
use crate::text::{
    FontLoadingError, Fonts, NativeTextMeasurer, ShapedText, SystemFontError, TextMeasurer,
};
use crate::text_input::ClipboardCommand;
use crate::ui::{Env, ImeTarget, InputKey, PasteTarget, Ui};
use font_kit::{family_name::FamilyName, font::Font, properties::Properties, source::SystemSource};
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use std::{fmt, io};
use winit::application::ApplicationHandler;
use winit::cursor::CursorIcon;
use winit::data_transfer::{DataTransferId, DataTransferSendBuilder, TypeHint, TypedData};
use winit::event::{ElementState, Ime, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::event_loop::{
    ActiveEventLoop, AsyncRequestSerial, ControlFlow, DndAction, EventLoop, EventLoopProxy,
};
use winit::keyboard::{Key as WinitKey, KeyCode, NamedKey, PhysicalKey};
use winit::window::{
    ImeCapabilities, ImeEnableRequest, ImeRequest, ImeRequestData, ImeSurroundingText,
    Window as WinitWindow, WindowAttributes, WindowId,
};

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
type View<S> = Box<dyn FnMut(&mut S, &mut Context<S>) -> Node>;

/// Application-wide state handed to the `run` closure and to global action handlers.
pub struct AppContext {
    pub(crate) keymap: Keymap,
    pub(crate) images: ImageStore,
    pending: Vec<Box<dyn PendingWindow>>,
    globals: Vec<(TypeId, GlobalHandler)>,
    quit: bool,
    measurer: Option<TextMeasurerFactory>,
    fonts: Fonts,
    default_font: FontId,
    shape_text: bool,
    text_renderer: Option<TextRendererFactory>,
    proxy: Option<EventLoopProxy>,
}

impl AppContext {
    fn new() -> Self {
        Self {
            keymap: Keymap::default(),
            images: ImageStore::default(),
            pending: Vec::new(),
            globals: Vec::new(),
            quit: false,
            measurer: None,
            fonts: Fonts::default(),
            default_font: FontId::DEFAULT,
            shape_text: false,
            text_renderer: None,
            proxy: None,
        }
    }

    /// Registers key bindings.
    pub fn bind_keys(&mut self, bindings: impl IntoIterator<Item = KeyBinding>) {
        for b in bindings {
            self.keymap.add(b);
        }
    }

    /// Handles action `A` when no node on the focus path did.
    pub fn on_action<A: Action + Event>(
        &mut self,
        handler: impl Fn(&A, &mut AppContext) + 'static,
    ) {
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

    /// Registers PNG or JPEG bytes for background decoding on first use.
    pub fn register_image(&mut self, bytes: impl Into<Vec<u8>>) -> io::Result<ImageId> {
        let bytes = bytes.into();
        self.register_image_supplier(move || Ok(bytes))
    }

    /// Registers a one-shot PNG or JPEG source. Both loading and decoding run off the UI
    /// thread. The image is empty until decoding succeeds. A failed source stays empty.
    pub fn register_image_supplier(
        &mut self,
        source: impl FnOnce() -> io::Result<Vec<u8>> + Send + 'static,
    ) -> io::Result<ImageId> {
        let proxy = self
            .proxy
            .clone()
            .ok_or_else(|| io::Error::other("no running event loop"))?;
        self.images.register_encoded(Box::new(source), proxy)
    }

    /// Loads the first face of a TrueType (.ttf) or OpenType (.otf) file.
    /// Use the returned ID with `Text::font` or `set_default_font`.
    pub fn load_font(&mut self, path: impl AsRef<Path>) -> Result<FontId, FontLoadingError> {
        Font::from_path(path, 0).map(|font| self.fonts.insert(font))
    }

    /// Loads a face by index from in-memory TrueType or OpenType font data.
    pub fn load_font_bytes(
        &mut self,
        bytes: impl Into<Arc<Vec<u8>>>,
        index: u32,
    ) -> Result<FontId, FontLoadingError> {
        Font::from_bytes(bytes.into(), index).map(|font| self.fonts.insert(font))
    }

    /// Loads an installed face by PostScript name, or the regular face of a named family.
    pub fn load_system_font(&mut self, name: &str) -> Result<FontId, SystemFontError> {
        let source = SystemSource::new();
        let handle = source
            .select_by_postscript_name(name)
            .or_else(|_| {
                source.select_best_match(&[FamilyName::Title(name.to_owned())], &Properties::new())
            })
            .map_err(SystemFontError::Selection)?;
        let font = handle.load().map_err(SystemFontError::Loading)?;
        Ok(self.fonts.insert(font))
    }

    /// Sets the face used for `FontId::DEFAULT` by the built-in text backend in new windows.
    /// Pass `FontId::DEFAULT` to restore the system sans-serif face.
    pub fn set_default_font(&mut self, font: FontId) {
        self.default_font = font;
    }

    /// Enables Unicode shaping, bidirectional layout and font fallback in new windows.
    /// Custom text measurers and renderers keep taking precedence for display text.
    /// Text inputs always use the built-in shaping backend.
    pub fn enable_text_shaping(&mut self) {
        self.shape_text = true;
    }

    /// Replaces the text measurer used by new windows (real text shaping plugs in here).
    pub fn set_text_measurer(&mut self, f: impl Fn() -> Box<dyn TextMeasurer> + 'static) {
        self.measurer = Some(Box::new(f));
    }
    /// Replaces the text renderer used by new windows.
    pub fn set_text_renderer(&mut self, f: impl Fn() -> Box<dyn TextRenderer> + 'static) {
        self.text_renderer = Some(Box::new(f));
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
        view: impl FnMut(&mut S, &mut Context<S>) -> Node + 'static,
    ) {
        self.pending.push(Box::new(WindowSpec {
            options,
            state,
            view: Box::new(view),
        }));
    }

    fn route_global(&mut self, a: Box<dyn Action>) {
        let tid = a.type_id_of();
        let handlers: Vec<GlobalHandler> = self
            .globals
            .iter()
            .filter(|(t, _)| *t == tid)
            .map(|(_, h)| h.clone())
            .collect();
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
    fn images_changed(&mut self);
    fn wake_at(&mut self, now: Instant) -> Option<Instant>;
    fn clipboard_received(
        &mut self,
        app: &mut AppContext,
        id: DataTransferId,
        serial: AsyncRequestSerial,
        value: &dyn TypedData,
    ) -> bool;
}

struct WindowSpec<S> {
    options: WindowOptions,
    state: S,
    view: View<S>,
}

struct FileTransfer {
    serial: Option<AsyncRequestSerial>,
    position: Option<Vec2>,
    drop: Option<(u64, Vec2)>,
    paths: Option<Vec<PathBuf>>,
}

struct PendingPaste {
    id: DataTransferId,
    serial: AsyncRequestSerial,
    target: PasteTarget,
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
    view: View<S>,
    measurer: Box<dyn TextMeasurer>,
    clear: Color,
    dirty: bool,
    last_frame: Instant,
    redraw_at: Option<Instant>,
    transfers: HashMap<DataTransferId, FileTransfer>,
    paste: Option<PendingPaste>,
    ime: Option<ImeTarget>,
    text_cursor: bool,
    closed: bool,
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
            .with_surface_size(winit::dpi::LogicalSize::new(
                o.size.x as f64,
                o.size.y as f64,
            ))
            .with_resizable(o.resizable);
        if let Some(m) = o.min_size {
            attrs =
                attrs.with_min_surface_size(winit::dpi::LogicalSize::new(m.x as f64, m.y as f64));
        }
        let win: Arc<dyn WinitWindow> = Arc::from(el.create_window(attrs)?);

        let (gpu_ctx, surface) =
            match gpu {
                Some(g) => {
                    let s = g.instance.create_surface(
                        wgpu::SurfaceTarget::from_window_without_display(win.clone()),
                    )?;
                    (g.clone(), s)
                }
                None => {
                    let instance = wgpu::Instance::new(
                        wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
                            el.owned_display_handle(),
                        )),
                    );
                    let s = instance.create_surface(
                        wgpu::SurfaceTarget::from_window_without_display(win.clone()),
                    )?;
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
        renderer.set_native_fonts(NativeTextMeasurer::with_fonts(
            app.fonts.clone(),
            app.default_font,
        ));
        let input_text = ShapedText::new(app.fonts.clone(), app.default_font);
        let shaped = app.shape_text.then(|| input_text.clone());
        renderer.set_input_text(input_text.clone());
        if let Some(text) = &shaped {
            renderer.set_shaped_text(text.clone());
        }
        if let Some(factory) = &app.text_renderer {
            renderer.set_text_renderer(factory());
        }

        let mut ui = Ui::new();
        ui.input_text = input_text;
        ui.set_window_focus(win.has_focus());
        let scale = win.scale_factor() as f32;
        ui.set_size(
            Vec2::new(size.width as f32 / scale, size.height as f32 / scale),
            scale,
        );

        let mut cx = Context::new();
        cx.info = WindowInfo {
            size: ui.size,
            scale,
            focus: None,
            hover: Vec::new(),
        };
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
            measurer: match &app.measurer {
                Some(factory) => factory(),
                None => match shaped {
                    Some(text) => Box::new(text),
                    None => Box::new(NativeTextMeasurer::with_fonts(
                        app.fonts.clone(),
                        app.default_font,
                    )),
                },
            },
            clear: self.options.clear_color,
            dirty: true,
            last_frame: Instant::now(),
            redraw_at: None,
            transfers: HashMap::new(),
            paste: None,
            ime: None,
            text_cursor: false,
            closed: false,
        }))
    }
}

fn map_key(code: KeyCode) -> Option<Key> {
    use KeyCode as K;
    Some(match code {
        K::KeyA => Key::A,
        K::KeyB => Key::B,
        K::KeyC => Key::C,
        K::KeyD => Key::D,
        K::KeyE => Key::E,
        K::KeyF => Key::F,
        K::KeyG => Key::G,
        K::KeyH => Key::H,
        K::KeyI => Key::I,
        K::KeyJ => Key::J,
        K::KeyK => Key::K,
        K::KeyL => Key::L,
        K::KeyM => Key::M,
        K::KeyN => Key::N,
        K::KeyO => Key::O,
        K::KeyP => Key::P,
        K::KeyQ => Key::Q,
        K::KeyR => Key::R,
        K::KeyS => Key::S,
        K::KeyT => Key::T,
        K::KeyU => Key::U,
        K::KeyV => Key::V,
        K::KeyW => Key::W,
        K::KeyX => Key::X,
        K::KeyY => Key::Y,
        K::KeyZ => Key::Z,
        K::Digit0 => Key::Num0,
        K::Digit1 => Key::Num1,
        K::Digit2 => Key::Num2,
        K::Digit3 => Key::Num3,
        K::Digit4 => Key::Num4,
        K::Digit5 => Key::Num5,
        K::Digit6 => Key::Num6,
        K::Digit7 => Key::Num7,
        K::Digit8 => Key::Num8,
        K::Digit9 => Key::Num9,
        K::F1 => Key::F1,
        K::F2 => Key::F2,
        K::F3 => Key::F3,
        K::F4 => Key::F4,
        K::F5 => Key::F5,
        K::F6 => Key::F6,
        K::F7 => Key::F7,
        K::F8 => Key::F8,
        K::F9 => Key::F9,
        K::F10 => Key::F10,
        K::F11 => Key::F11,
        K::F12 => Key::F12,
        K::Enter | K::NumpadEnter => Key::Enter,
        K::Escape => Key::Escape,
        K::Space => Key::Space,
        K::Tab => Key::Tab,
        K::Backspace => Key::Backspace,
        K::Delete => Key::Delete,
        K::Insert => Key::Insert,
        K::Home => Key::Home,
        K::End => Key::End,
        K::PageUp => Key::PageUp,
        K::PageDown => Key::PageDown,
        K::ArrowUp => Key::Up,
        K::ArrowDown => Key::Down,
        K::ArrowLeft => Key::Left,
        K::ArrowRight => Key::Right,
        K::Minus => Key::Minus,
        K::Equal => Key::Equal,
        K::Comma => Key::Comma,
        K::Period => Key::Period,
        K::Slash => Key::Slash,
        K::Backslash => Key::Backslash,
        K::Semicolon => Key::Semicolon,
        K::Quote => Key::Quote,
        K::Backquote => Key::Backquote,
        K::BracketLeft => Key::BracketLeft,
        K::BracketRight => Key::BracketRight,
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

fn editor_key(event: &KeyEvent, physical: Option<Key>) -> Option<Key> {
    use NamedKey as N;
    match &event.logical_key {
        WinitKey::Named(named) => Some(match named {
            N::ArrowLeft => Key::Left,
            N::ArrowRight => Key::Right,
            N::ArrowUp => Key::Up,
            N::ArrowDown => Key::Down,
            N::Home => Key::Home,
            N::End => Key::End,
            N::PageUp => Key::PageUp,
            N::PageDown => Key::PageDown,
            N::Backspace => Key::Backspace,
            N::Delete => Key::Delete,
            N::Insert => Key::Insert,
            N::Enter => Key::Enter,
            N::Tab => Key::Tab,
            N::Escape => Key::Escape,
            N::Copy => Key::Copy,
            N::Cut => Key::Cut,
            N::Paste => Key::Paste,
            N::Undo => Key::Undo,
            N::Redo => Key::Redo,
            _ => return physical,
        }),
        _ => match &event.key_without_modifiers {
            WinitKey::Character(c) => Key::from_name(c).or_else(|| {
                // A layout without Latin letters can still use the conventional physical shortcuts.
                physical.filter(|key| {
                    key.name().len() == 1 && key.name().as_bytes()[0].is_ascii_alphabetic()
                })
            }),
            _ => physical,
        },
    }
}

/// Converts a `file://` URI into a path.
pub(crate) fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let url = url::Url::parse(uri).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path().ok()
}

impl<S: 'static> Window<S> {
    fn clipboard(&mut self, el: &dyn ActiveEventLoop, command: ClipboardCommand) {
        let cut = matches!(command, ClipboardCommand::Cut(_));
        match command {
            ClipboardCommand::Copy(value) | ClipboardCommand::Cut(value) => {
                let transfer = DataTransferSendBuilder::new(value)
                    .with_type(TypeHint::Plaintext, |text, _| Some(text.to_string()))
                    .build();
                if el.set_clipboard(transfer).is_ok() && cut {
                    let mut env = Env {
                        state: &mut self.state,
                        cx: &mut self.cx,
                    };
                    self.ui.cut(&mut env);
                }
            }
            ClipboardCommand::Paste => {
                self.paste = None;
                let Some(target) = self.ui.paste_target() else {
                    return;
                };
                let Ok(Some(id)) = el.clipboard() else { return };
                if el
                    .data_transfer(id)
                    .is_ok_and(|data| data.has_type(&TypeHint::Plaintext))
                    && let Ok(serial) = el.fetch_data_transfer(id, &TypeHint::Plaintext)
                {
                    self.paste = Some(PendingPaste { id, serial, target });
                }
            }
        }
    }

    fn input_cursor(&mut self) {
        let text = self.ui.text_cursor();
        if self.text_cursor != text {
            self.text_cursor = text;
            self.win.set_cursor(
                if text {
                    CursorIcon::Text
                } else {
                    CursorIcon::Default
                }
                .into(),
            );
        }
    }

    fn sync_ime(&mut self) {
        let target = self.ui.ime_target();
        if target == self.ime {
            return;
        }
        let previous = std::mem::replace(&mut self.ime, target);
        let same_editor = self
            .ime
            .as_ref()
            .zip(previous.as_ref())
            .is_some_and(|(target, old)| target.key == old.key);
        if !same_editor && previous.is_some() {
            let _ = self.win.request_ime_update(ImeRequest::Disable);
        }
        if let Some(target) = &self.ime {
            let rect = target.rect;
            let mut data = ImeRequestData::default().with_cursor_area(
                winit::dpi::LogicalPosition::new(rect.x, rect.y).into(),
                winit::dpi::LogicalSize::new(rect.w, rect.h).into(),
            );
            let limit = ImeSurroundingText::MAX_TEXT_BYTES - 1;
            let anchor = if target.caret.abs_diff(target.anchor) < limit {
                target.anchor
            } else {
                target.caret
            };
            let left = target.caret.min(anchor);
            let right = target.caret.max(anchor);
            let available = limit - (right - left);
            let start = target
                .text
                .ceil_char_boundary(left.saturating_sub(available / 2));
            let end = target
                .text
                .floor_char_boundary((start + limit).min(target.text.len()));
            let selection_anchor = if anchor <= end { anchor } else { target.caret };
            if target.caret <= end
                && let Ok(text) = ImeSurroundingText::new(
                    target.text[start..end].to_owned(),
                    target.caret - start,
                    selection_anchor - start,
                )
            {
                data = data.with_surrounding_text(text);
            }
            let request = if same_editor {
                ImeRequest::Update(data)
            } else {
                let caps = ImeCapabilities::new()
                    .with_cursor_area()
                    .with_surrounding_text();
                ImeRequest::Enable(ImeEnableRequest::new(caps, data).unwrap())
            };
            let _ = self.win.request_ime_update(request);
        }
    }

    fn sync_info(&mut self) {
        self.cx.info = WindowInfo {
            size: self.ui.size,
            scale: self.ui.scale,
            focus: self.ui.focused_key(),
            hover: self.ui.hover_keys(),
        };
    }

    fn dispatch_focus_changes(&mut self) {
        if !self.ui.focus_changes.is_empty() {
            self.sync_info();
            let mut env = Env {
                state: &mut self.state,
                cx: &mut self.cx,
            };
            self.ui.dispatch_focus_changes(&mut env);
        }
    }

    /// Applies effects requested by handlers until none are left.
    fn pump(&mut self, app: &mut AppContext) {
        self.dispatch_focus_changes();
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
                    Effect::CloseWindow => self.closed = true,
                    Effect::Focus(id) => {
                        self.ui.set_focus(Some(&id));
                        self.dispatch_focus_changes();
                        self.dirty = true;
                    }
                    Effect::Blur => {
                        self.ui.set_focus(None);
                        self.dispatch_focus_changes();
                        self.dirty = true;
                    }
                    Effect::SetTitle(t) => self.win.set_title(&t),
                    Effect::Dispatch(a) => {
                        self.sync_info();
                        let handled = {
                            let mut env = Env {
                                state: &mut self.state,
                                cx: &mut self.cx,
                            };
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

    fn start_drag(&mut self, el: &dyn ActiveEventLoop, position: Vec2) {
        if let Some(data) = self.ui.drag_data(position)
            && el
                .start_drag(self.win.id(), data, &[DndAction::Copy], None)
                .is_ok()
        {
            self.ui.drag_started();
        }
    }

    fn drag_position(
        &mut self,
        el: &dyn ActiveEventLoop,
        id: DataTransferId,
        position: Option<Vec2>,
    ) {
        if let Some(transfer) = self.transfers.get_mut(&id)
            && transfer.drop.is_none()
        {
            transfer.position = position;
        }
        let accepts = self
            .transfers
            .get(&id)
            .and_then(|t| t.position)
            .and_then(|p| self.ui.drop_target(p))
            .is_some();
        let actions: &[DndAction] = if accepts { &[DndAction::Copy] } else { &[] };
        let _ = el.set_valid_dnd_actions(id, actions);
    }

    fn finish_drop(&mut self, app: &mut AppContext, id: DataTransferId) {
        let Some(transfer) = self.transfers.get_mut(&id) else {
            return;
        };
        let Some((target, position)) = transfer.drop else {
            return;
        };
        let Some(paths) = transfer.paths.take() else {
            return;
        };
        self.transfers.remove(&id);
        if !paths.is_empty() {
            self.sync_info();
            let mut env = Env {
                state: &mut self.state,
                cx: &mut self.cx,
            };
            self.ui
                .file_drop(target, FileDropEvent { paths, position }, &mut env);
            self.pump(app);
        }
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
        self.ui.set_size(
            Vec2::new(size.width as f32 / scale, size.height as f32 / scale),
            scale,
        );
        self.win.request_redraw();
    }

    fn redraw(&mut self, app: &mut AppContext) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }
        let now = Instant::now();
        let elapsed = (now - self.last_frame).as_secs_f32();
        let dt = elapsed.min(0.05);
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

        let (animating, geometry_changed) = {
            let input_text = self.ui.input_text.clone();
            let mut lenv = LayoutEnv {
                text: &mut *self.measurer,
                input_text: Some(&input_text),
                images: &mut app.images,
            };
            self.ui.frame(dt, elapsed, &mut lenv)
        };

        // Re-evaluate hover against the fresh layout (content may have moved under the pointer).
        if rebuilt || geometry_changed {
            self.sync_info();
            let mut env = Env {
                state: &mut self.state,
                cx: &mut self.cx,
            };
            if self.ui.pointer_moved(None, &mut env) {
                self.win.request_redraw();
            }
            self.pump(app);
        }
        self.input_cursor();
        self.sync_ime();

        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
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

        self.redraw_at = self
            .ui
            .tooltip_wait()
            .into_iter()
            .chain(self.ui.input_wait())
            .min()
            .and_then(|wait| now.checked_add(wait));
        if animating || self.dirty {
            self.win.request_redraw();
        }
    }
}

impl<S: 'static> UiWindow for Window<S> {
    fn id(&self) -> WindowId {
        self.win.id()
    }

    fn images_changed(&mut self) {
        self.ui.invalidate_layout();
        self.win.request_redraw();
    }

    fn wake_at(&mut self, now: Instant) -> Option<Instant> {
        let deadline = self.redraw_at?;
        if deadline <= now {
            self.redraw_at = None;
            self.win.request_redraw();
            return None;
        }
        Some(deadline)
    }

    fn clipboard_received(
        &mut self,
        app: &mut AppContext,
        id: DataTransferId,
        serial: AsyncRequestSerial,
        value: &dyn TypedData,
    ) -> bool {
        let Some(paste) = self
            .paste
            .take_if(|paste| paste.id == id && paste.serial == serial)
        else {
            return false;
        };
        if let Ok(text) = value.try_as_string() {
            self.sync_info();
            let mut env = Env {
                state: &mut self.state,
                cx: &mut self.cx,
            };
            if self.ui.paste(paste.target, &text, &mut env) {
                self.win.request_redraw();
            }
            self.pump(app);
        }
        true
    }

    fn event(&mut self, app: &mut AppContext, el: &dyn ActiveEventLoop, ev: WindowEvent) -> bool {
        match ev {
            WindowEvent::CloseRequested => return false,
            WindowEvent::SurfaceResized(_) => self.resize(),
            WindowEvent::ScaleFactorChanged { .. } => self.resize(),
            WindowEvent::RedrawRequested => self.redraw(app),
            WindowEvent::Occluded(false) => self.win.request_redraw(),

            WindowEvent::PointerMoved {
                position,
                primary: true,
                ..
            } => {
                let p = self.logical(position);
                self.start_drag(el, p);
                self.sync_info();
                let changed = {
                    let mut env = Env {
                        state: &mut self.state,
                        cx: &mut self.cx,
                    };
                    self.ui.pointer_moved(Some(p), &mut env)
                };
                if changed {
                    self.win.request_redraw();
                }
                self.input_cursor();
                self.pump(app);
            }
            WindowEvent::PointerLeft {
                position,
                primary: true,
                ..
            } => {
                if let Some(position) = position {
                    self.start_drag(el, self.logical(position));
                }
                self.sync_info();
                {
                    let mut env = Env {
                        state: &mut self.state,
                        cx: &mut self.cx,
                    };
                    self.ui.pointer_left(&mut env);
                }
                self.win.request_redraw();
                self.input_cursor();
                self.pump(app);
            }
            WindowEvent::PointerButton {
                state,
                position,
                primary: true,
                button,
                ..
            } => {
                if let Some(b) = button.mouse_button() {
                    let p = self.logical(position);
                    self.sync_info();
                    {
                        let mut env = Env {
                            state: &mut self.state,
                            cx: &mut self.cx,
                        };
                        self.ui.pointer_button(
                            map_button(b),
                            state == ElementState::Pressed,
                            Some(p),
                            &mut env,
                        );
                    }
                    self.win.request_redraw();
                    self.pump(app);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        Vec2::new(x * LINE_SCROLL_PX, y * LINE_SCROLL_PX)
                    }
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
            WindowEvent::Focused(focused) => {
                if !focused {
                    self.ui.mods = Modifiers::NONE;
                    self.paste = None;
                }
                self.ui.set_window_focus(focused);
                self.pump(app);
                self.sync_ime();
                self.win.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } if event.state == ElementState::Pressed => {
                let key = match event.physical_key {
                    PhysicalKey::Code(c) => map_key(c),
                    _ => None,
                };
                let text: Option<Arc<str>> = event.text.as_ref().map(|t| Arc::from(t.as_str()));
                self.sync_info();
                let mut env = Env {
                    state: &mut self.state,
                    cx: &mut self.cx,
                };
                let edit = if is_synthetic {
                    InputKey::Unhandled
                } else {
                    self.ui
                        .input_key(editor_key(&event, key), text.as_deref(), &mut env)
                };
                match edit {
                    InputKey::Unhandled => {
                        self.ui
                            .key_down(key, text, event.repeat, &app.keymap, &mut env)
                    }
                    InputKey::Handled => self.win.request_redraw(),
                    InputKey::Clipboard(command) => {
                        self.clipboard(el, command);
                        self.win.request_redraw();
                    }
                }
                self.pump(app);
            }
            WindowEvent::Ime(ime) => {
                self.sync_info();
                match ime {
                    Ime::Preedit(value, cursor) => self.ui.preedit(&value, cursor),
                    Ime::Commit(value) => {
                        let mut env = Env {
                            state: &mut self.state,
                            cx: &mut self.cx,
                        };
                        self.ui.commit_input(&value, &mut env);
                    }
                    Ime::DeleteSurrounding {
                        before_bytes,
                        after_bytes,
                    } => {
                        let mut env = Env {
                            state: &mut self.state,
                            cx: &mut self.cx,
                        };
                        self.ui
                            .delete_surrounding(before_bytes, after_bytes, &mut env);
                    }
                    Ime::Disabled => self.ui.preedit("", None),
                    _ => {}
                }
                self.win.request_redraw();
                self.pump(app);
            }

            // ---- file drag & drop ----
            WindowEvent::DragEntered { id, position } => {
                if el
                    .data_transfer(id)
                    .is_ok_and(|t| t.has_type(&TypeHint::UriList))
                {
                    let serial = el.fetch_data_transfer(id, &TypeHint::UriList).ok();
                    self.transfers.insert(
                        id,
                        FileTransfer {
                            serial,
                            position: None,
                            drop: None,
                            paths: None,
                        },
                    );
                }
                self.drag_position(el, id, position.map(|p| self.logical(p)));
            }
            WindowEvent::DragPosition { id, position, .. } => {
                self.drag_position(el, id, Some(self.logical(position)));
            }
            WindowEvent::DataTransferReceived { id, serial, value } => {
                if let Some(transfer) = self.transfers.get_mut(&id)
                    && transfer.serial == Some(serial)
                {
                    transfer.paths = Some(
                        value
                            .try_as_uris()
                            .ok()
                            .into_iter()
                            .flatten()
                            .filter_map(|uri| uri_to_path(&uri))
                            .collect(),
                    );
                    self.finish_drop(app, id);
                }
            }
            WindowEvent::DragDropped { id, .. } => {
                if let Some(transfer) = self.transfers.get_mut(&id) {
                    transfer.drop = transfer
                        .position
                        .and_then(|p| self.ui.drop_target(p).map(|key| (key, p)));
                    if transfer.drop.is_some() && transfer.serial.is_none() {
                        transfer.serial = el.fetch_data_transfer(id, &TypeHint::UriList).ok();
                    }
                    if transfer.drop.is_some() && transfer.serial.is_some() {
                        self.finish_drop(app, id);
                    } else {
                        self.transfers.remove(&id);
                    }
                }
            }
            WindowEvent::DragLeft { id } => {
                self.transfers.remove(&id);
            }
            _ => {}
        }
        !self.closed
    }
}

// ---------------------------------------------------------------------------------------------

struct Runner {
    init: Option<Box<dyn FnOnce(&mut AppContext)>>,
    app: AppContext,
    gpu: Option<Rc<GpuContext>>,
    windows: HashMap<WindowId, Box<dyn UiWindow>>,
    error: Rc<RefCell<Option<RunError>>>,
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
                    self.error.borrow_mut().get_or_insert(RunError::Window(e));
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
        if self.app.quit || self.windows.is_empty() {
            el.exit();
        }
    }

    fn window_event(&mut self, el: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // Clipboard replies can be delivered to a window other than the requesting one.
        let clipboard = if let WindowEvent::DataTransferReceived { id, serial, value } = &event {
            self.windows.values_mut().any(|window| {
                window.clipboard_received(&mut self.app, *id, *serial, value.as_ref())
            })
        } else {
            false
        };
        let keep = clipboard
            || self
                .windows
                .get_mut(&id)
                .is_none_or(|window| window.event(&mut self.app, el, event));
        if !keep {
            self.windows.remove(&id);
        }
        self.open_pending(el);
        if self.app.quit || self.windows.is_empty() {
            el.exit();
        }
    }

    fn proxy_wake_up(&mut self, _: &dyn ActiveEventLoop) {
        if self.app.images.resolve_decoded() {
            for window in self.windows.values_mut() {
                window.images_changed();
            }
        }
    }

    fn about_to_wait(&mut self, el: &dyn ActiveEventLoop) {
        let now = Instant::now();
        let next = self
            .windows
            .values_mut()
            .filter_map(|window| window.wake_at(now))
            .min();
        el.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}

/// An error starting the event loop or creating a window.
#[derive(Debug)]
pub enum RunError {
    EventLoop(winit::error::EventLoopError),
    Window(Box<dyn std::error::Error>),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventLoop(error) => write!(f, "window event loop: {error}"),
            Self::Window(error) => write!(f, "could not open window: {error}"),
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EventLoop(error) => Some(error),
            Self::Window(error) => Some(error.as_ref()),
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
        if let Err(error) = self.try_run(init) {
            eprintln!("octlui: {error}");
        }
    }

    /// Runs the event loop and returns any startup or window creation error to the caller.
    pub fn try_run(self, init: impl FnOnce(&mut AppContext) + 'static) -> Result<(), RunError> {
        let event_loop = EventLoop::new().map_err(RunError::EventLoop)?;
        let error = Rc::new(RefCell::new(None));
        let mut app = AppContext::new();
        app.proxy = Some(event_loop.create_proxy());
        let runner = Runner {
            init: Some(Box::new(init)),
            app,
            gpu: None,
            windows: HashMap::new(),
            error: error.clone(),
        };
        event_loop.run_app(runner).map_err(RunError::EventLoop)?;
        error.borrow_mut().take().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loaded_fonts_resolve_as_default_and_explicit_faces() {
        let source = SystemSource::new();
        let Ok(handle) = source.select_best_match(&[FamilyName::Monospace], &Properties::new())
        else {
            return;
        };
        let font = handle.load().unwrap();
        let name = font.postscript_name().unwrap_or_else(|| font.family_name());
        let mut app = AppContext::new();
        let system = app.load_system_font(&name).unwrap();
        let selected = match handle {
            font_kit::handle::Handle::Path {
                path,
                font_index: 0,
            } => app.load_font(path).unwrap(),
            _ => system,
        };
        app.set_default_font(selected);
        let mut measurer = NativeTextMeasurer::with_fonts(app.fonts.clone(), app.default_font);
        let face = measurer.face(FontId::DEFAULT).unwrap();
        assert_eq!(face.id, selected);
        assert_eq!(face.font.postscript_name(), font.postscript_name());
        assert_eq!(measurer.face(system).unwrap().id, system);
    }

    #[test]
    fn uris() {
        #[cfg(unix)]
        assert_eq!(
            uri_to_path("file://localhost/home/a%20b/c.png"),
            Some(PathBuf::from("/home/a b/c.png"))
        );
        #[cfg(windows)]
        assert_eq!(
            uri_to_path("file://localhost/C:/a%20b/y.txt"),
            Some(PathBuf::from(r"C:\a b\y.txt"))
        );
        assert_eq!(uri_to_path("http://x"), None);
    }
}
