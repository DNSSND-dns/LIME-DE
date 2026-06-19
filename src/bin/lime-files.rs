use std::{
    fs,
    path::{Path, PathBuf},
};

use lime_de::lime_ui::{LimeUiTheme, PixelCanvas};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_output, delegate_pointer, delegate_registry, delegate_seat,
    delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        xdg::{
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
        },
        WaylandSurface,
    },
    shm::{
        slot::{Buffer, SlotPool},
        Shm, ShmHandler,
    },
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

const INITIAL_WIDTH: u32 = 820;
const INITIAL_HEIGHT: u32 = 560;
const TOOLBAR_HEIGHT: i32 = 58;
const ROW_HEIGHT: i32 = 38;
const BACK_BUTTON_WIDTH: i32 = 54;
const LEFT_PADDING: i32 = 18;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init(&connection)?;
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh)?;
    let xdg_shell = XdgShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let pool = SlotPool::new(INITIAL_WIDTH as usize * INITIAL_HEIGHT as usize * 4, &shm)?;
    let surface = compositor.create_surface(&qh);
    let window = xdg_shell.create_window(surface, WindowDecorations::None, &qh);
    window.set_title("LIME Files");
    window.set_app_id("lime-files");
    window.set_min_size(Some((480, 320)));
    window.commit();

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    let mut app = LimeFiles {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool,
        buffer: None,
        window,
        pointer: None,
        exit: false,
        configured: false,
        width: INITIAL_WIDTH,
        height: INITIAL_HEIGHT,
        cursor: (0.0, 0.0),
        current_dir: home,
        entries: Vec::new(),
        scroll: 0,
        status: String::new(),
        theme: LimeUiTheme::load(),
    };
    app.reload();

    while !app.exit {
        event_queue.blocking_dispatch(&mut app)?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct FileEntry {
    path: PathBuf,
    name: String,
    directory: bool,
}

struct LimeFiles {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    buffer: Option<Buffer>,
    window: Window,
    pointer: Option<wl_pointer::WlPointer>,
    exit: bool,
    configured: bool,
    width: u32,
    height: u32,
    cursor: (f64, f64),
    current_dir: PathBuf,
    entries: Vec<FileEntry>,
    scroll: usize,
    status: String,
    theme: LimeUiTheme,
}

impl LimeFiles {
    fn reload(&mut self) {
        self.entries.clear();
        self.status.clear();

        match fs::read_dir(&self.current_dir) {
            Ok(read_dir) => {
                self.entries = read_dir
                    .filter_map(Result::ok)
                    .filter_map(|entry| {
                        let file_type = entry.file_type().ok()?;
                        Some(FileEntry {
                            path: entry.path(),
                            name: entry.file_name().to_string_lossy().into_owned(),
                            directory: file_type.is_dir(),
                        })
                    })
                    .collect();
                self.entries.sort_by(|left, right| {
                    right
                        .directory
                        .cmp(&left.directory)
                        .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                });
            }
            Err(error) => self.status = format!("Cannot open: {error}"),
        }
    }

    fn visible_rows(&self) -> usize {
        ((self.height as i32 - TOOLBAR_HEIGHT - 20).max(ROW_HEIGHT) / ROW_HEIGHT) as usize
    }

    fn activate_at(&mut self, x: f64, y: f64) {
        if y < f64::from(TOOLBAR_HEIGHT) {
            if x < f64::from(BACK_BUTTON_WIDTH) {
                if let Some(parent) = self.current_dir.parent() {
                    self.current_dir = parent.to_path_buf();
                    self.scroll = 0;
                    self.reload();
                }
            }
            return;
        }

        let row = ((y as i32 - TOOLBAR_HEIGHT) / ROW_HEIGHT).max(0) as usize + self.scroll;
        let Some(entry) = self.entries.get(row).cloned() else {
            return;
        };
        if entry.directory {
            self.current_dir = entry.path;
            self.scroll = 0;
            self.reload();
        } else {
            self.status = format!("File: {}", entry.name);
        }
    }

    fn scroll_by(&mut self, delta: i32) {
        let maximum = self.entries.len().saturating_sub(self.visible_rows());
        self.scroll = if delta < 0 {
            self.scroll.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            self.scroll.saturating_add(delta as usize).min(maximum)
        };
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured || self.width == 0 || self.height == 0 {
            return;
        }

        let row_count = self.visible_rows();
        let stride = self.width as i32 * 4;
        let buffer = self.buffer.get_or_insert_with(|| {
            self.pool
                .create_buffer(
                    self.width as i32,
                    self.height as i32,
                    stride,
                    wl_shm::Format::Argb8888,
                )
                .expect("create LIME Files SHM buffer")
                .0
        });
        let canvas = match self.pool.canvas(buffer) {
            Some(canvas) => canvas,
            None => {
                let (next_buffer, canvas) = self
                    .pool
                    .create_buffer(
                        self.width as i32,
                        self.height as i32,
                        stride,
                        wl_shm::Format::Argb8888,
                    )
                    .expect("create second LIME Files SHM buffer");
                *buffer = next_buffer;
                canvas
            }
        };
        let mut pixels = PixelCanvas::new(canvas, self.width, self.height);
        pixels.clear(self.theme.background);
        pixels.rect(0, 0, self.width as i32, TOOLBAR_HEIGHT, self.theme.surface);
        pixels.rounded_rect(10, 10, 38, 38, self.theme.radius, self.theme.surface_hover);
        pixels.text(23, 21, "<", self.theme.text);
        pixels.text(64, 20, &short_path(&self.current_dir, 72), self.theme.text);

        for (visible_index, entry) in self
            .entries
            .iter()
            .skip(self.scroll)
            .take(row_count)
            .enumerate()
        {
            let y = TOOLBAR_HEIGHT + visible_index as i32 * ROW_HEIGHT;
            let hovered =
                self.cursor.1 >= f64::from(y) && self.cursor.1 < f64::from(y + ROW_HEIGHT);
            if hovered {
                pixels.rounded_rect(
                    8,
                    y + 2,
                    self.width as i32 - 16,
                    ROW_HEIGHT - 4,
                    self.theme.radius,
                    self.theme.surface_hover,
                );
            }
            let icon_color = if entry.directory {
                self.theme.accent
            } else {
                self.theme.file
            };
            pixels.rounded_rect(LEFT_PADDING, y + 10, 18, 16, 4, icon_color);
            pixels.text(
                LEFT_PADDING + 31,
                y + 11,
                &truncate(&entry.name, 64),
                self.theme.text,
            );
        }

        if !self.status.is_empty() {
            pixels.rect(
                0,
                self.height as i32 - 28,
                self.width as i32,
                28,
                self.theme.surface,
            );
            pixels.text(
                LEFT_PADDING,
                self.height as i32 - 21,
                &truncate(&self.status, 72),
                self.theme.muted_text,
            );
        }

        self.window
            .wl_surface()
            .damage_buffer(0, 0, self.width as i32, self.height as i32);
        buffer
            .attach_to(self.window.wl_surface())
            .expect("attach LIME Files buffer");
        self.window.commit();
        let _ = qh;
    }
}

impl CompositorHandler for LimeFiles {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.draw(qh);
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl WindowHandler for LimeFiles {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &Window,
        configure: WindowConfigure,
        _: u32,
    ) {
        self.width = configure
            .new_size
            .0
            .map(|value| value.get())
            .unwrap_or(INITIAL_WIDTH);
        self.height = configure
            .new_size
            .1
            .map(|value| value.get())
            .unwrap_or(INITIAL_HEIGHT);
        self.buffer = None;
        self.configured = true;
        self.draw(qh);
    }
}

impl SeatHandler for LimeFiles {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = self.seat_state.get_pointer(qh, &seat).ok();
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            if let Some(pointer) = self.pointer.take() {
                pointer.release();
            }
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for LimeFiles {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if &event.surface != self.window.wl_surface() {
                continue;
            }
            self.cursor = event.position;
            match event.kind {
                PointerEventKind::Press { button: 0x110, .. } => {
                    self.activate_at(event.position.0, event.position.1);
                }
                PointerEventKind::Axis { vertical, .. } => {
                    let amount = if vertical.discrete != 0 {
                        f64::from(vertical.discrete)
                    } else {
                        vertical.absolute
                    };
                    if amount != 0.0 {
                        self.scroll_by(if amount > 0.0 { 3 } else { -3 });
                    }
                }
                _ => {}
            }
        }
        self.draw(qh);
    }
}

impl OutputHandler for LimeFiles {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ShmHandler for LimeFiles {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(LimeFiles);
delegate_output!(LimeFiles);
delegate_shm!(LimeFiles);
delegate_seat!(LimeFiles);
delegate_pointer!(LimeFiles);
delegate_xdg_shell!(LimeFiles);
delegate_xdg_window!(LimeFiles);
delegate_registry!(LimeFiles);

impl ProvidesRegistryState for LimeFiles {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

fn short_path(path: &Path, maximum: usize) -> String {
    truncate(&path.to_string_lossy(), maximum)
}

fn truncate(text: &str, maximum: usize) -> String {
    let mut value = text.chars().take(maximum).collect::<String>();
    if text.chars().count() > maximum {
        value.push_str("...");
    }
    value
}
