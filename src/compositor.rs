use std::{
    env,
    os::fd::OwnedFd,
    process::{Child, Command},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use smithay::{
    backend::input::{ButtonState, KeyState, Keycode},
    backend::renderer::buffer_dimensions,
    delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_shell,
    input::{
        keyboard::{FilterResult, KeyboardHandle, XkbConfig},
        pointer::{ButtonEvent, CursorImageStatus, MotionEvent, PointerHandle},
        Seat, SeatHandler, SeatState,
    },
    output::{Mode as SmithayMode, Output as SmithayOutput, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::EventLoop,
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::{wl_buffer, wl_callback, wl_seat, wl_shm, wl_surface::WlSurface},
            Client, Display, DisplayHandle, Resource,
        },
    },
    utils::{Logical, Point, Serial, Size, SERIAL_COUNTER},
    wayland::{
        buffer::BufferHandler,
        compositor::{
            self as smithay_compositor, BufferAssignment, CompositorClientState, CompositorHandler,
            CompositorState as SmithayCompositorState, SurfaceAttributes,
        },
        output::{OutputHandler, OutputManagerState},
        selection::{
            data_device::{
                set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState,
                ServerDndGrabHandler,
            },
            SelectionHandler,
        },
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceData,
        },
        shm::{ShmHandler, ShmState},
        socket::ListeningSocketSource,
    },
};

use crate::{
    animation::{AnimationKind, AnimationManager, Easing},
    backend::{WinitBackend, WinitBackendOutputEvent, WinitMouseButton},
    client_buffer::read_shm_pixels,
    config::{AnimationConfig, BehaviorConfig, StyleConfig},
    dock::{Dock, DockItemId, DockStyle},
    error::AppError,
    input::CursorState,
    output::Output,
    panel::{Panel, PanelStyle},
    render::{
        RenderBackend, RenderCircle, RenderColor, RenderImage, RenderRect, RenderRoundedRect,
        RenderSceneFrame, RenderText,
    },
    scene::{Scene, SceneNodeId},
    window::{
        ClientBufferMetadata, ClientBufferPixels, Window, WindowButtonGeometry, WindowDecoration,
        WindowGeometry, WindowId,
    },
};

#[derive(Debug)]
pub struct Compositor {
    initialized: bool,
    running: Arc<AtomicBool>,
    use_winit_test_backend: bool,
    launch_test_client: bool,
    test_client_commands: Vec<String>,
    style: StyleConfig,
    behavior: BehaviorConfig,
    animations_config: AnimationConfig,
    test_client: Option<Child>,
    winit_backend: Option<WinitBackend>,
    wayland: Option<WaylandRuntime>,
}

#[derive(Debug)]
pub struct CompositorState {
    display_handle: DisplayHandle,
    compositor_state: SmithayCompositorState,
    seat_state: SeatState<Self>,
    seat: Seat<Self>,
    pointer: PointerHandle<Self>,
    keyboard: KeyboardHandle<Self>,
    shm_state: ShmState,
    data_device_state: DataDeviceState,
    xdg_shell_state: XdgShellState,
    output_manager_state: OutputManagerState,
    style: StyleConfig,
    behavior: BehaviorConfig,
    animations_config: AnimationConfig,
    outputs: Vec<Output>,
    wayland_outputs: Vec<SmithayOutput>,
    render_backend: RenderBackend,
    scene: Scene,
    dock: Dock,
    panel: Panel,
    animations: AnimationManager,
    output_scene_nodes: Vec<SceneNodeId>,
    windows: Vec<TrackedWindow>,
    z_order: Vec<WindowId>,
    next_window_id: u64,
    start_time: Instant,
    frame_counter: u64,
    last_frame_time: Instant,
    last_memory_debug_time: Instant,
    last_render_command_count: usize,
    last_frame_submitted_at: Option<Instant>,
    target_frame_interval: Duration,
    dirty: bool,
    awaiting_frame_presentation: bool,
    cursor: CursorState,
    hovered_window: Option<WindowId>,
    active_window: Option<WindowId>,
    interaction: PointerInteraction,
    pending_launch_commands: Vec<Vec<String>>,
}

impl CompositorState {
    #[must_use]
    fn new(
        display_handle: DisplayHandle,
        style: StyleConfig,
        behavior: BehaviorConfig,
        animations_config: AnimationConfig,
    ) -> Self {
        let compositor_state = SmithayCompositorState::new::<Self>(&display_handle);
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&display_handle, "seat-0");
        let pointer = seat.add_pointer();
        let keyboard = seat
            .add_keyboard(XkbConfig::default(), 250, 25)
            .expect("default xkb keymap should initialize");
        let shm_state = ShmState::new::<Self>(
            &display_handle,
            [wl_shm::Format::Abgr8888, wl_shm::Format::Xbgr8888],
        );
        let data_device_state = DataDeviceState::new::<Self>(&display_handle);
        let xdg_shell_state = XdgShellState::new::<Self>(&display_handle);
        let output_manager_state = OutputManagerState::new();
        let outputs = vec![Output::virtual_default()];
        let wayland_outputs = outputs
            .iter()
            .map(|output| create_wayland_output(&display_handle, output))
            .collect::<Vec<_>>();
        let cursor = CursorState::centered(outputs[0].width, outputs[0].height);
        println!("Cursor initialized");
        let render_backend = RenderBackend::new();
        let mut scene = Scene::new();
        let mut dock = Dock::new(DockStyle::from_config(&style.dock));
        let mut panel = Panel::new(PanelStyle::from_config(&style.panel));
        let mut output_scene_nodes = Vec::new();

        for output in &outputs {
            println!(
                "Output created: {} {}x{}",
                output.name, output.width, output.height
            );
            output_scene_nodes.push(scene.add_output(output.id));
        }
        dock.layout(outputs[0].width, outputs[0].height);
        panel.layout(outputs[0].width, outputs[0].height);

        Self {
            display_handle,
            compositor_state,
            seat_state,
            seat,
            pointer,
            keyboard,
            shm_state,
            data_device_state,
            xdg_shell_state,
            output_manager_state,
            style,
            behavior,
            animations_config,
            outputs,
            wayland_outputs,
            render_backend,
            scene,
            dock,
            panel,
            animations: AnimationManager::new(),
            output_scene_nodes,
            windows: Vec::new(),
            z_order: Vec::new(),
            next_window_id: 1,
            start_time: Instant::now(),
            frame_counter: 0,
            last_frame_time: Instant::now(),
            last_memory_debug_time: Instant::now(),
            last_render_command_count: 0,
            last_frame_submitted_at: None,
            target_frame_interval: Duration::from_millis(16),
            dirty: true,
            awaiting_frame_presentation: false,
            cursor,
            hovered_window: None,
            active_window: None,
            interaction: PointerInteraction::None,
            pending_launch_commands: Vec::new(),
        }
    }

    fn insert_client(&mut self, stream: std::os::unix::net::UnixStream) -> Result<(), AppError> {
        self.display_handle
            .insert_client(stream, Arc::new(ClientState::default()))
            .map(|_| ())
            .map_err(|error| AppError::new(format!("failed to insert Wayland client: {error}")))
    }

    fn output_count(&self) -> usize {
        let _seat_global = self.seat.global();
        let _output_manager = &self.output_manager_state;
        let _advertised_outputs = self.wayland_outputs.len();
        self.outputs.len()
    }

    fn primary_output_size(&self) -> (u32, u32) {
        self.outputs
            .first()
            .map(|output| (output.width, output.height))
            .expect("LIME DE compositor state should always have a primary output")
    }

    fn geometry_for_output(&self, geometry: WindowGeometry) -> WindowGeometry {
        let (width, height) = self.primary_output_size();
        geometry.with_default_for_output(width, height)
    }

    fn window_decoration(&self) -> WindowDecoration {
        WindowDecoration::from_style(&self.style.window)
    }

    fn color_from_config(&self, hex: &str, fallback: RenderColor) -> RenderColor {
        RenderColor::from_hex_or(hex, fallback)
    }

    fn background_color(&self) -> RenderColor {
        self.color_from_config(&self.style.colors.background, RenderColor::black())
    }

    fn window_frame_color(&self) -> RenderColor {
        self.color_from_config(&self.style.colors.window_frame, RenderColor::window_frame())
    }

    fn titlebar_color(&self) -> RenderColor {
        self.color_from_config(&self.style.colors.titlebar, RenderColor::titlebar())
    }

    fn border_focused_color(&self) -> RenderColor {
        self.color_from_config(
            &self.style.colors.border_focused,
            RenderColor::focused_border(),
        )
    }

    fn title_text_color(&self) -> RenderColor {
        self.color_from_config(&self.style.colors.title_text, RenderColor::title_text())
    }

    fn placeholder_window_color(&self) -> RenderColor {
        self.color_from_config(
            &self.style.colors.placeholder_window,
            RenderColor::window_placeholder(),
        )
    }

    fn cursor_color(&self) -> RenderColor {
        self.color_from_config(&self.style.colors.cursor, RenderColor::white())
    }

    fn close_button_color(&self) -> RenderColor {
        self.color_from_config(
            &self.style.colors.close_button,
            RenderColor::from_rgb_u8(255, 95, 87),
        )
    }

    fn minimize_button_color(&self) -> RenderColor {
        self.color_from_config(
            &self.style.colors.minimize_button,
            RenderColor::from_rgb_u8(255, 189, 46),
        )
    }

    fn maximize_button_color(&self) -> RenderColor {
        self.color_from_config(
            &self.style.colors.maximize_button,
            RenderColor::from_rgb_u8(40, 200, 64),
        )
    }

    fn dock_bubble_color(&self) -> RenderColor {
        self.color_from_config(
            &self.style.colors.dock_bubble,
            RenderColor::from_rgb_u8(32, 38, 41),
        )
    }

    fn dock_text_color(&self) -> RenderColor {
        self.color_from_config(&self.style.colors.dock_text, RenderColor::title_text())
    }

    fn panel_background_color(&self) -> RenderColor {
        self.color_from_config(
            &self.style.colors.panel_background,
            RenderColor::from_rgb_u8(32, 38, 41),
        )
    }

    fn panel_text_color(&self) -> RenderColor {
        self.color_from_config(&self.style.colors.panel_text, RenderColor::title_text())
    }

    fn animations_enabled(&self) -> bool {
        self.animations_config.enabled
    }

    fn default_window_geometry(
        &self,
        cascade_index: usize,
        size: Option<(i32, i32)>,
    ) -> WindowGeometry {
        let decoration = self.window_decoration();
        let (output_width, output_height) = self.primary_output_size();
        let output_width = output_width as i32;
        let output_height = output_height as i32;
        let (width, height) = size.map_or_else(
            || {
                (
                    ((output_width as f64) * 0.50).round() as i32,
                    ((output_height as f64) * 0.45).round() as i32,
                )
            },
            |(client_width, client_height)| {
                decoration.total_size(client_width.max(1), client_height.max(1))
            },
        );
        let width = width.max(1);
        let height = height.max(1);
        let cascade_offset = cascade_index as i32 * 32;
        let x = ((output_width - width) / 2) + cascade_offset;
        let y = ((output_height - height) / 2) + cascade_offset;
        let mut geometry = WindowGeometry {
            x,
            y,
            width,
            height,
        };

        if let Some(output) = self.outputs.first() {
            clamp_window_to_output(&mut geometry, output);
        }

        geometry
    }

    fn set_window_geometry(&mut self, index: usize, geometry: WindowGeometry) {
        self.windows[index].window.geometry = geometry;
        println!(
            "Window geometry set: {} {} {} {} {}",
            self.windows[index].window.id, geometry.x, geometry.y, geometry.width, geometry.height
        );
    }

    fn update_window_size_from_buffer(&mut self, index: usize, width: i32, height: i32) -> bool {
        if width <= 0 || height <= 0 {
            return false;
        }

        let buffer_size = (width, height);
        let pending_configure_size = self.windows[index].window.pending_configure_size;
        let window_id = self.windows[index].window.id;
        let is_locked =
            self.windows[index].window.maximized || self.windows[index].window.minimized;
        self.windows[index].window.client_size = Some(buffer_size);

        if pending_configure_size.is_some() {
            self.windows[index].window.pending_configure_size = None;
        }
        if is_locked {
            return false;
        }
        if self.is_resizing_window(window_id)
            && !self
                .behavior
                .windows
                .accept_client_geometry_during_live_resize
        {
            return false;
        }
        if self.windows[index].window.user_resized
            && self.behavior.windows.keeps_user_resized_geometry_fixed()
        {
            return false;
        }
        if !self.windows[index].window.user_resized
            && !self
                .behavior
                .windows
                .allow_client_geometry_before_user_resize
        {
            return false;
        }

        let decoration = self.window_decoration();
        let (total_width, total_height) = decoration.total_size(width, height);
        let mut geometry = self.windows[index].window.geometry;
        if geometry.width == total_width && geometry.height == total_height {
            return false;
        }

        geometry.width = total_width;
        geometry.height = total_height;
        if let Some(output) = self.outputs.first() {
            clamp_window_to_output(&mut geometry, output);
        }
        self.windows[index].window.geometry = geometry;
        println!(
            "Window geometry updated: {} {} {} {} {}",
            self.windows[index].window.id, geometry.x, geometry.y, geometry.width, geometry.height
        );

        true
    }

    fn client_size_for_geometry(&self, geometry: WindowGeometry) -> (i32, i32) {
        let decoration = self.window_decoration();
        let width = (geometry.width - decoration.border_width * 2).max(1);
        let height =
            (geometry.height - decoration.titlebar_height - decoration.border_width).max(1);

        (width, height)
    }

    fn configure_window_client_size(&mut self, window_id: WindowId) {
        let Some(index) = self.window_index_for_id(window_id) else {
            return;
        };
        let size = self.client_size_for_geometry(self.windows[index].window.geometry);
        let surface = self.windows[index].surface.clone();

        self.windows[index].window.pending_configure_size = Some(size);

        surface.with_pending_state(|state| {
            state.size = Some(Size::<i32, Logical>::from(size));
        });
        surface.send_configure();
    }

    fn sync_primary_output_size(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        let Some(output) = self.outputs.first_mut() else {
            return;
        };

        if !output.resize(width, height) {
            return;
        }

        if let Some(wayland_output) = self.wayland_outputs.first() {
            update_wayland_output(wayland_output, output);
        }

        self.cursor
            .move_to(self.cursor.x, self.cursor.y, width, height);
        self.dock.layout(width, height);
        self.panel.layout(width, height);
        self.dock.update_hover(self.cursor.x, self.cursor.y);
        self.panel.update_hover(self.cursor.x, self.cursor.y);
        let output = self.outputs[0].clone();
        let mut geometry_changed = false;
        for tracked in &mut self.windows {
            let previous_geometry = tracked.window.geometry;
            clamp_window_to_output(&mut tracked.window.geometry, &output);
            if tracked.window.geometry != previous_geometry {
                geometry_changed = true;
                println!(
                    "Window geometry updated: {} {} {} {} {}",
                    tracked.window.id,
                    tracked.window.geometry.x,
                    tracked.window.geometry.y,
                    tracked.window.geometry.width,
                    tracked.window.geometry.height
                );
            }
        }
        self.update_pointer_focus();
        if geometry_changed {
            self.request_redraw();
        }
        self.request_redraw();
    }

    fn render_frame(&mut self) -> RenderSceneFrame {
        let now = Instant::now();
        let delta_ms = now.duration_since(self.last_frame_time).as_millis() as u64;
        self.frame_counter += 1;
        self.last_frame_time = now;
        self.animations.update(delta_ms);

        println!("Frame {}", self.frame_counter);

        let mut scene_frame = RenderSceneFrame::new(self.background_color());
        let (output_width, output_height) = self.primary_output_size();
        let _scene_window_geometries = self
            .scene
            .window_geometries_for_output(output_width, output_height);

        for window_id in self.z_order.clone() {
            let Some(tracked) = self.window_for_id(window_id) else {
                continue;
            };
            let animation_frame = self.animations.frame_for_window(window_id);
            if !tracked.window.mapped || (tracked.window.minimized && animation_frame.is_none()) {
                continue;
            }

            let geometry = animation_frame.as_ref().map_or_else(
                || {
                    tracked
                        .window
                        .geometry
                        .with_default_for_output(output_width, output_height)
                },
                |frame| frame.rect,
            );
            let decoration = self.window_decoration();
            let border_width = decoration
                .border_width
                .min(geometry.width)
                .min(geometry.height);
            let corner_radius = animation_frame
                .as_ref()
                .map_or(decoration.corner_radius, |frame| frame.radius);
            let bottom_corner_radius = animation_frame
                .as_ref()
                .map_or(decoration.bottom_corner_radius, |frame| frame.radius);
            let content_source = if animation_frame.as_ref().is_some_and(|frame| {
                matches!(
                    frame.kind,
                    AnimationKind::MinimizeToDock | AnimationKind::RestoreFromDock
                )
            }) {
                tracked
                    .window
                    .animation_client_pixels
                    .as_ref()
                    .or(tracked.window.cached_client_pixels.as_ref())
                    .or(tracked.window.client_pixels.as_ref())
            } else {
                tracked.window.client_pixels.as_ref()
            };

            scene_frame.push_rounded_rect(RenderRoundedRect::with_vertical_radii(
                geometry.x,
                geometry.y,
                geometry.width,
                geometry.height,
                corner_radius,
                bottom_corner_radius,
                self.border_focused_color(),
            ));
            scene_frame.push_rounded_rect(RenderRoundedRect::with_vertical_radii(
                geometry.x + border_width,
                geometry.y + border_width,
                (geometry.width - border_width * 2).max(1),
                (geometry.height - border_width * 2).max(1),
                (corner_radius - border_width).max(0),
                (bottom_corner_radius - border_width).max(0),
                self.window_frame_color(),
            ));
            scene_frame.push_rounded_rect(RenderRoundedRect::with_vertical_radii(
                geometry.x + border_width,
                geometry.y + border_width,
                (geometry.width - border_width * 2).max(1),
                (decoration.titlebar_height - border_width).max(1),
                (corner_radius - border_width).max(0),
                0,
                self.titlebar_color(),
            ));

            let (client_x, client_y) = decoration.client_origin(geometry);
            let client_width = (geometry.width - decoration.border_width * 2).max(1);
            let client_height =
                (geometry.height - decoration.titlebar_height - decoration.border_width).max(1);
            let window_clip = RenderRoundedRect::with_vertical_radii(
                geometry.x + border_width,
                geometry.y + border_width,
                (geometry.width - border_width * 2).max(1),
                (geometry.height - border_width * 2).max(1),
                (corner_radius - border_width).max(0),
                (bottom_corner_radius - border_width).max(0),
                self.background_color(),
            );
            let close_button = decoration.close_button(geometry);
            let minimize_button = decoration.minimize_button(geometry);
            let maximize_button = decoration.maximize_button(geometry);
            scene_frame.push_circle(RenderCircle::new(
                close_button.x,
                close_button.y,
                close_button.diameter,
                self.close_button_color(),
            ));
            scene_frame.push_circle(RenderCircle::new(
                minimize_button.x,
                minimize_button.y,
                minimize_button.diameter,
                self.minimize_button_color(),
            ));
            scene_frame.push_circle(RenderCircle::new(
                maximize_button.x,
                maximize_button.y,
                maximize_button.diameter,
                self.maximize_button_color(),
            ));
            if let Some(title) = tracked.window.title.as_deref() {
                scene_frame.push_text(RenderText::new(
                    geometry.x + 82,
                    geometry.y + 9,
                    title,
                    self.title_text_color(),
                ));
            }

            if let Some(client_pixels) = content_source {
                let image = RenderImage::new(
                    client_x,
                    client_y,
                    client_pixels.width,
                    client_pixels.height,
                    client_pixels.pixels_argb.clone(),
                );
                let image = if animation_frame.is_some()
                    || self.behavior.windows.fits_client_buffer_to_window()
                {
                    image.fit_to(client_width as u32, client_height as u32)
                } else {
                    image.clipped_to(client_width as u32, client_height as u32)
                };
                scene_frame.push_image(image.with_clip(window_clip));
            } else {
                let client_geometry = WindowGeometry {
                    x: client_x,
                    y: client_y,
                    width: client_width,
                    height: client_height,
                };
                scene_frame.push_rect(RenderRect {
                    x: client_geometry.x,
                    y: client_geometry.y,
                    width: client_geometry.width,
                    height: client_geometry.height,
                    color: self.placeholder_window_color(),
                });
            }
        }

        let dock_bubble_color = self.dock_bubble_color();
        let dock_text_color = self.dock_text_color();
        let dock_radius = self.dock.style.bubble_radius;
        for item in self.dock.items() {
            let radius = dock_radius.min(item.size / 2).max(0);
            scene_frame.push_rounded_rect(RenderRoundedRect::with_vertical_radii(
                item.x,
                item.y,
                item.size,
                item.size,
                radius,
                radius,
                dock_bubble_color,
            ));

            if let Some(initial) = item.label.chars().next() {
                scene_frame.push_text(RenderText::new(
                    item.x + item.size / 2 - 3,
                    item.y + item.size / 2 - 5,
                    initial.to_string(),
                    dock_text_color,
                ));
            }
        }

        // Render top panel (macOS-style menu bar)
        let panel_background_color = self.panel_background_color();
        let panel_text_color = self.panel_text_color();
        let panel_radius = self.panel.style.radius;

        // Draw panel background
        scene_frame.push_rounded_rect(RenderRoundedRect::with_vertical_radii(
            0,
            0,
            output_width as i32,
            self.panel.height(),
            panel_radius,
            panel_radius,
            panel_background_color,
        ));

        // Render panel items
        for item in self.panel.items() {
            if let Some(icon) = &item.icon {
                // Simple icon representation using first character
                if let Some(initial) = icon.chars().next() {
                    scene_frame.push_text(RenderText::new(
                        item.x + item.width / 2 - 3,
                        item.y + item.height / 2 - 5,
                        initial.to_string(),
                        panel_text_color,
                    ));
                }
            } else {
                // Fallback to label if no icon
                if let Some(initial) = item.label.chars().next() {
                    scene_frame.push_text(RenderText::new(
                        item.x + item.width / 2 - 3,
                        item.y + item.height / 2 - 5,
                        initial.to_string(),
                        panel_text_color,
                    ));
                }
            }
        }

        self.finish_window_animations();

        if self.cursor.visible {
            let cursor_color = self.cursor_color();
            scene_frame
                .cursor
                .push(RenderRect::cursor_horizontal_with_color(
                    self.cursor.x as i32,
                    self.cursor.y as i32,
                    cursor_color,
                ));
            scene_frame
                .cursor
                .push(RenderRect::cursor_vertical_with_color(
                    self.cursor.x as i32,
                    self.cursor.y as i32,
                    cursor_color,
                ));
        }

        for output in &self.outputs {
            self.render_backend.begin_frame(output);
            self.render_backend.clear(scene_frame.clear_color);
            for rectangle in &scene_frame.rectangles {
                self.render_backend.draw_rect(*rectangle);
            }
            for cursor in &scene_frame.cursor {
                self.render_backend.draw_rect(*cursor);
            }
            self.render_backend.finish_frame();
        }

        self.last_render_command_count = scene_frame.commands.len();
        self.mark_clean();
        if self.animations.has_active() {
            self.dirty = true;
        }

        scene_frame
    }

    fn should_render_frame(&self) -> bool {
        !self.awaiting_frame_presentation
            && self.last_frame_time.elapsed() >= self.target_frame_interval
    }

    fn mark_frame_submitted(&mut self) {
        self.awaiting_frame_presentation = true;
        self.last_frame_submitted_at = Some(Instant::now());
    }

    fn mark_frame_presented(&mut self) {
        self.awaiting_frame_presentation = false;
        self.last_frame_submitted_at = None;
    }

    fn recover_stalled_frame_presentation(&mut self) {
        let Some(submitted_at) = self.last_frame_submitted_at else {
            return;
        };
        if !self.awaiting_frame_presentation || submitted_at.elapsed() < Duration::from_millis(250)
        {
            return;
        }

        eprintln!("Winit frame presentation timed out; allowing redraw recovery");
        self.awaiting_frame_presentation = false;
        self.last_frame_submitted_at = None;
        self.request_redraw();
    }

    fn request_redraw(&mut self) {
        if !self.dirty {
            println!("Redraw requested");
        }

        self.dirty = true;
    }

    fn needs_redraw(&self) -> bool {
        self.dirty
    }

    fn mark_clean(&mut self) {
        self.dirty = false;
    }

    fn log_memory_debug_tick(&mut self) {
        if self.last_memory_debug_time.elapsed() < Duration::from_secs(1) {
            return;
        }

        let (cached_buffers, cached_pixel_bytes) = self.cached_client_buffer_stats();
        println!(
            "Memory debug: frames={}, windows={}, scene_nodes={}, render_rects={}, cached_buffers={}, cached_pixel_bytes={}, awaiting_present={}",
            self.frame_counter,
            self.windows.len(),
            self.scene.node_count(),
            self.last_render_command_count,
            cached_buffers,
            cached_pixel_bytes,
            self.awaiting_frame_presentation
        );
        self.last_memory_debug_time = Instant::now();
    }

    fn cached_client_buffer_stats(&self) -> (usize, usize) {
        self.windows.iter().fold((0, 0), |(count, bytes), tracked| {
            if let Some(pixels) = tracked.window.cached_client_pixels.as_ref() {
                (count + 1, bytes + pixels.byte_len())
            } else {
                (count, bytes)
            }
        })
    }

    fn take_pending_launch_commands(&mut self) -> Vec<Vec<String>> {
        std::mem::take(&mut self.pending_launch_commands)
    }

    fn create_window(&mut self, surface: ToplevelSurface) -> WindowId {
        let id = WindowId::new(self.next_window_id);
        self.next_window_id += 1;

        let (title, app_id) = Self::surface_metadata(&surface);
        let mut window = Window::new(id);
        window.title = title;
        window.app_id = app_id;

        self.windows.push(TrackedWindow {
            surface,
            window,
            scene_node: None,
            frame_callbacks: Vec::new(),
        });
        self.z_order.push(id);
        let index = self.windows.len() - 1;
        let geometry = self.default_window_geometry(index, None);
        self.set_window_geometry(index, geometry);

        id
    }

    fn update_window_title(&mut self, surface: &ToplevelSurface) -> Option<WindowId> {
        let (title, _) = Self::surface_metadata(surface);
        let tracked = self.window_for_surface_mut(surface)?;

        tracked.window.title = title;

        Some(tracked.window.id)
    }

    fn update_window_app_id(&mut self, surface: &ToplevelSurface) -> Option<WindowId> {
        let (_, app_id) = Self::surface_metadata(surface);
        let tracked = self.window_for_surface_mut(surface)?;

        tracked.window.app_id = app_id;

        Some(tracked.window.id)
    }

    fn update_window_mapping_from_commit(&mut self, surface: &WlSurface) {
        let surface_commit = smithay_compositor::with_states(surface, |states| {
            let mut attributes = states.cached_state.get::<SurfaceAttributes>();
            let current = attributes.current();

            let buffer = current.buffer.as_ref().map(|buffer| match buffer {
                BufferAssignment::NewBuffer(buffer) => {
                    let size = buffer_dimensions(buffer).map(|size| (size.w, size.h));
                    let pixels = read_shm_pixels(buffer);
                    CommittedBuffer::Attached { size, pixels }
                }
                BufferAssignment::Removed => CommittedBuffer::Removed,
            });

            SurfaceCommit {
                buffer,
                frame_callbacks: std::mem::take(&mut current.frame_callbacks),
            }
        });

        let Some(index) = self.window_index_for_wl_surface(surface) else {
            return;
        };

        if !surface_commit.frame_callbacks.is_empty() {
            self.windows[index]
                .frame_callbacks
                .extend(surface_commit.frame_callbacks);
        }

        let Some(committed_buffer) = surface_commit.buffer else {
            return;
        };

        let mapped = committed_buffer.is_attached();
        let mut redraw_requested = false;

        match committed_buffer {
            CommittedBuffer::Attached { size, pixels } => {
                let metadata = size
                    .map_or_else(ClientBufferMetadata::unknown_size, |(width, height)| {
                        ClientBufferMetadata::from_size(width, height)
                    });

                self.windows[index].window.client_buffer = Some(metadata);
                if let Some((width, height)) = size {
                    self.update_window_size_from_buffer(index, width, height);
                }
                println!("Window buffer attached: {}", self.windows[index].window.id);
                if let Some(pixels) = pixels {
                    println!(
                        "Window SHM buffer readable: {} {}x{}",
                        self.windows[index].window.id, pixels.width, pixels.height
                    );
                    self.windows[index].window.cached_client_pixels = Some(pixels.clone());
                    self.windows[index].window.client_pixels = Some(pixels);
                } else {
                    println!(
                        "Window SHM buffer unsupported: {}",
                        self.windows[index].window.id
                    );
                    self.clear_window_client_buffers(index);
                }
                redraw_requested = true;
            }
            CommittedBuffer::Removed => {
                if self.windows[index].window.client_buffer.take().is_some() {
                    println!("Window buffer removed: {}", self.windows[index].window.id);
                    redraw_requested = true;
                }
                self.clear_window_client_buffers(index);
            }
        }

        if self.windows[index].window.mapped != mapped {
            self.windows[index].window.mapped = mapped;

            if mapped {
                self.windows[index].window.minimized = false;
                println!("Window mapped: {}", self.windows[index].window.id);
                self.create_window_scene_node(surface);
                self.start_open_window_animation(index);
            } else {
                println!("Window unmapped: {}", self.windows[index].window.id);
                self.clear_window_client_buffers(index);
                self.windows[index].frame_callbacks.clear();
                self.remove_window_scene_node(surface);
            }

            redraw_requested = true;
        }

        if redraw_requested {
            self.request_redraw();
        }
    }

    fn handle_backend_event(&mut self, event: WinitBackendOutputEvent) {
        match event {
            WinitBackendOutputEvent::MouseMoved { x, y } => {
                let Some(output) = self.outputs.first() else {
                    return;
                };
                let previous_cursor = self.cursor;
                self.cursor.move_to(x, y, output.width, output.height);

                if self.cursor != previous_cursor {
                    if self.dock.update_hover(self.cursor.x, self.cursor.y) {
                        self.dock.layout(output.width, output.height);
                    }
                    if self.is_window_interaction_active() {
                        self.update_window_interaction();
                    } else {
                        self.update_pointer_focus();
                    }
                    self.request_redraw();
                }
            }
            WinitBackendOutputEvent::MouseButton { button, pressed } => {
                let was_interacting = self.is_window_interaction_active();
                let released_interaction = self.interaction;
                let mut consumed_by_compositor = false;

                if button == WinitMouseButton::Left && pressed {
                    if self.handle_dock_button_press() {
                        consumed_by_compositor = true;
                    } else if let Some(window_id) =
                        self.hit_test_window(self.cursor.x, self.cursor.y)
                    {
                        if self.handle_window_button_press(window_id) {
                            self.request_redraw();
                            consumed_by_compositor = true;
                        } else {
                            self.focus_window(window_id);
                            consumed_by_compositor = self.start_window_interaction(window_id);
                        }
                    }
                }
                if button == WinitMouseButton::Left && !pressed {
                    self.interaction = PointerInteraction::None;
                    consumed_by_compositor = was_interacting;
                    if let PointerInteraction::Resize { window_id, .. } = released_interaction {
                        if self.behavior.windows.send_configure_on_resize_release {
                            self.configure_window_client_size(window_id);
                        }
                    }
                    self.update_pointer_focus();
                }

                if !consumed_by_compositor {
                    self.send_pointer_button(button, pressed);
                }
                self.request_redraw();
            }
            WinitBackendOutputEvent::Keyboard { keycode, pressed } => {
                self.send_keyboard_key(keycode, pressed);
            }
            WinitBackendOutputEvent::Resized { width, height } => {
                self.sync_primary_output_size(width, height);
            }
            WinitBackendOutputEvent::FramePresented => {
                self.mark_frame_presented();
                self.send_frame_callbacks();
            }
        }
    }

    fn clear_window_client_buffers(&mut self, index: usize) {
        self.windows[index].window.client_pixels = None;
        self.windows[index].window.cached_client_pixels = None;
        self.windows[index].window.animation_client_pixels = None;
    }

    fn send_frame_callbacks(&mut self) {
        let timestamp = self.frame_timestamp();

        for tracked in &mut self.windows {
            let callbacks = std::mem::take(&mut tracked.frame_callbacks);
            for callback in callbacks {
                callback.done(timestamp);
            }
        }
    }

    fn frame_timestamp(&self) -> u32 {
        self.start_time.elapsed().as_millis() as u32
    }

    fn remove_window(&mut self, surface: &ToplevelSurface) -> Option<WindowId> {
        let index = self
            .windows
            .iter()
            .position(|tracked| tracked.surface == *surface)?;

        let mut tracked = self.windows.remove(index);
        tracked.window.client_pixels = None;
        tracked.window.cached_client_pixels = None;
        tracked.window.animation_client_pixels = None;
        tracked.frame_callbacks.clear();
        self.z_order
            .retain(|window_id| *window_id != tracked.window.id);
        if self.hovered_window == Some(tracked.window.id) {
            self.hovered_window = None;
        }
        if self.active_window == Some(tracked.window.id) {
            self.active_window = None;
        }
        if let Some(scene_node_id) = tracked.scene_node {
            if self.scene.remove_node(scene_node_id).is_some() {
                println!("Scene node removed");
                self.request_redraw();
            }
        }

        Some(tracked.window.id)
    }

    fn hit_test_window(&self, x: f64, y: f64) -> Option<WindowId> {
        self.z_order.iter().rev().find_map(|window_id| {
            let tracked = self.window_for_id(*window_id)?;
            let geometry = self.geometry_for_output(tracked.window.geometry);

            (tracked.window.mapped
                && !tracked.window.minimized
                && !tracked.window.animating
                && x >= f64::from(geometry.x)
                && y >= f64::from(geometry.y)
                && x < f64::from(geometry.x + geometry.width)
                && y < f64::from(geometry.y + geometry.height))
            .then_some(*window_id)
        })
    }

    fn hit_test_client_window(&self, x: f64, y: f64) -> Option<WindowId> {
        self.z_order.iter().rev().find_map(|window_id| {
            let tracked = self.window_for_id(*window_id)?;
            let geometry = self.geometry_for_output(tracked.window.geometry);
            let decoration = self.window_decoration();
            let (client_x, client_y) = decoration.client_origin(geometry);
            let client_width = geometry.width - decoration.border_width * 2;
            let client_height =
                geometry.height - decoration.titlebar_height - decoration.border_width;

            (tracked.window.mapped
                && !tracked.window.minimized
                && !tracked.window.animating
                && x >= f64::from(client_x)
                && y >= f64::from(client_y)
                && x < f64::from(client_x + client_width)
                && y < f64::from(client_y + client_height))
            .then_some(*window_id)
        })
    }

    fn update_pointer_focus(&mut self) {
        let next_hovered = self.hit_test_client_window(self.cursor.x, self.cursor.y);

        if self.hovered_window == next_hovered {
            self.send_pointer_motion(next_hovered);
            return;
        }

        if let Some(window_id) = self.hovered_window {
            println!("Pointer left window: {window_id}");
        }
        if let Some(window_id) = next_hovered {
            println!("Pointer entered window: {window_id}");
        }

        self.hovered_window = next_hovered;
        self.send_pointer_motion(next_hovered);
    }

    fn focus_window(&mut self, window_id: WindowId) {
        let Some(window) = self.window_for_id(window_id).map(|tracked| &tracked.window) else {
            return;
        };
        if window.animating || window.minimized {
            return;
        }

        if self.active_window != Some(window_id) {
            self.active_window = Some(window_id);
            println!("Window focused: {window_id}");
        }

        self.bring_window_to_front(window_id);
        self.set_keyboard_focus(window_id);
        self.request_redraw();
    }

    fn bring_window_to_front(&mut self, window_id: WindowId) {
        self.z_order.retain(|id| *id != window_id);
        self.z_order.push(window_id);
    }

    fn resize_edge_for_window(&self, window_id: WindowId, x: f64, y: f64) -> Option<ResizeEdge> {
        let geometry = self.window_for_id(window_id)?.window.geometry;
        let geometry = self.geometry_for_output(geometry);
        let edge_size = 12.0;
        let right = (f64::from(geometry.x + geometry.width) - x).abs() <= edge_size;
        let bottom = (f64::from(geometry.y + geometry.height) - y).abs() <= edge_size;

        match (right, bottom) {
            (true, true) => Some(ResizeEdge::BottomRight),
            (true, false) => Some(ResizeEdge::Right),
            (false, true) => Some(ResizeEdge::Bottom),
            (false, false) => None,
        }
    }

    fn handle_window_button_press(&mut self, window_id: WindowId) -> bool {
        let Some(index) = self.window_index_for_id(window_id) else {
            return false;
        };
        let geometry = self.geometry_for_output(self.windows[index].window.geometry);
        let decoration = self.window_decoration();

        if button_hit(
            decoration.close_button(geometry),
            self.cursor.x,
            self.cursor.y,
        ) {
            self.close_window(window_id);
            return true;
        }
        if button_hit(
            decoration.maximize_button(geometry),
            self.cursor.x,
            self.cursor.y,
        ) {
            self.toggle_maximize_window(window_id);
            return true;
        }
        if button_hit(
            decoration.minimize_button(geometry),
            self.cursor.x,
            self.cursor.y,
        ) {
            self.minimize_window(window_id);
            return true;
        }

        false
    }

    fn handle_dock_button_press(&mut self) -> bool {
        let Some(item) = self.dock.item_at(self.cursor.x, self.cursor.y) else {
            return false;
        };
        let item_id = item.id;
        let label = item.label.clone();
        let commands = item.commands.clone();

        println!("Dock item activated: {label}");
        if let Some(window_id) = self.visible_window_for_dock_item(item_id) {
            self.minimize_window(window_id);
        } else if !self.restore_minimized_window_for_dock_item(item_id) {
            self.pending_launch_commands.push(commands);
            println!("Dock launch requested: {label}");
        }
        true
    }

    fn visible_window_for_dock_item(&self, item_id: DockItemId) -> Option<WindowId> {
        self.z_order.iter().rev().find_map(|window_id| {
            let tracked = self.window_for_id(*window_id)?;
            (tracked.window.mapped
                && !tracked.window.minimized
                && !tracked.window.animating
                && tracked
                    .window
                    .app_id
                    .as_deref()
                    .is_some_and(|app_id| self.dock.app_matches_item(item_id, app_id)))
            .then_some(*window_id)
        })
    }

    fn close_window(&mut self, window_id: WindowId) {
        let Some(index) = self.window_index_for_id(window_id) else {
            return;
        };
        if self.windows[index].window.animating {
            return;
        }
        if !self.animations_enabled() {
            self.windows[index].surface.send_close();
            return;
        }
        let app_id = self.windows[index].window.app_id.clone();
        let Some(to_rect) = self.dock.item_rect_for_app(app_id.as_deref()) else {
            self.windows[index].surface.send_close();
            return;
        };
        let decoration = self.window_decoration();
        let from_rect = self.geometry_for_output(self.windows[index].window.geometry);

        self.windows[index].window.animating = true;
        self.cache_animation_client_pixels(index);
        if self.hovered_window == Some(window_id) {
            self.hovered_window = None;
        }
        if self.active_window == Some(window_id) {
            self.active_window = None;
            self.clear_keyboard_focus();
        }
        self.interaction = PointerInteraction::None;
        self.animations.start_window_animation(
            AnimationKind::CloseToDock,
            window_id,
            from_rect,
            to_rect,
            u64::from(self.animations_config.window_close_ms),
            Easing::EaseInOutCubic,
            decoration.corner_radius,
            self.dock.style.bubble_radius,
        );

        println!("Close animation started: {window_id}");
        self.update_pointer_focus();
        self.request_redraw();
    }

    fn minimize_window(&mut self, window_id: WindowId) {
        let Some(index) = self.window_index_for_id(window_id) else {
            return;
        };
        if self.windows[index].window.animating || self.windows[index].window.minimized {
            return;
        }
        let app_id = self.windows[index].window.app_id.clone();
        let Some(to_rect) = self.dock.item_rect_for_app(app_id.as_deref()) else {
            return;
        };
        let decoration = self.window_decoration();
        let from_rect = self.geometry_for_output(self.windows[index].window.geometry);

        if !self.animations_enabled() {
            self.windows[index].window.minimized = true;
            self.dock.set_active_for_app(app_id.as_deref(), true);
            self.request_redraw();
            return;
        }

        self.windows[index].window.animating = true;
        self.cache_animation_client_pixels(index);
        if self.hovered_window == Some(window_id) {
            self.hovered_window = None;
        }
        if self.active_window == Some(window_id) {
            self.active_window = None;
            self.clear_keyboard_focus();
        }
        self.interaction = PointerInteraction::None;

        self.animations.start_window_animation(
            AnimationKind::MinimizeToDock,
            window_id,
            from_rect,
            to_rect,
            u64::from(self.animations_config.window_close_ms),
            Easing::EaseInOutCubic,
            decoration.corner_radius,
            self.dock.style.bubble_radius,
        );

        println!("Minimize animation started: {window_id}");
        self.update_pointer_focus();
        self.request_redraw();
    }

    fn restore_minimized_window_for_dock_item(&mut self, item_id: DockItemId) -> bool {
        let Some(index) = self.windows.iter().position(|tracked| {
            tracked.window.mapped
                && tracked.window.minimized
                && tracked
                    .window
                    .app_id
                    .as_deref()
                    .is_some_and(|app_id| self.dock.app_matches_item(item_id, app_id))
        }) else {
            return false;
        };
        let window_id = self.windows[index].window.id;
        if self.windows[index].window.animating {
            return false;
        }
        let app_id = self.windows[index].window.app_id.clone();
        let Some(from_rect) = self.dock.item_rect_for_app(app_id.as_deref()) else {
            return false;
        };
        let decoration = self.window_decoration();
        let to_rect = self.geometry_for_output(self.windows[index].window.geometry);

        if !self.animations_enabled() {
            self.windows[index].window.minimized = false;
            self.dock.set_active_for_app(app_id.as_deref(), false);
            self.focus_window(window_id);
            self.update_pointer_focus();
            return true;
        }

        self.windows[index].window.animating = true;
        self.cache_animation_client_pixels(index);
        self.bring_window_to_front(window_id);
        self.animations.start_window_animation(
            AnimationKind::RestoreFromDock,
            window_id,
            from_rect,
            to_rect,
            u64::from(self.animations_config.window_open_ms),
            Easing::EaseOutCubic,
            self.dock.style.bubble_radius,
            decoration.corner_radius,
        );

        println!("Restore animation started: {window_id}");
        self.update_pointer_focus();
        self.request_redraw();
        true
    }

    fn start_open_window_animation(&mut self, index: usize) {
        if !self.animations_enabled() || self.windows[index].window.animating {
            return;
        }
        let app_id = self.windows[index].window.app_id.clone();
        let Some(from_rect) = self.dock.item_rect_for_app(app_id.as_deref()) else {
            return;
        };
        let window_id = self.windows[index].window.id;
        let decoration = self.window_decoration();
        let to_rect = self.geometry_for_output(self.windows[index].window.geometry);

        self.windows[index].window.animating = true;
        self.cache_animation_client_pixels(index);
        self.animations.start_window_animation(
            AnimationKind::OpenFromDock,
            window_id,
            from_rect,
            to_rect,
            u64::from(self.animations_config.window_open_ms),
            Easing::EaseOutCubic,
            self.dock.style.bubble_radius,
            decoration.corner_radius,
        );

        println!("Open animation started: {window_id}");
        self.request_redraw();
    }

    fn cache_animation_client_pixels(&mut self, index: usize) {
        self.windows[index].window.animation_client_pixels = self.windows[index]
            .window
            .client_pixels
            .clone()
            .or_else(|| self.windows[index].window.cached_client_pixels.clone());
    }

    fn finish_window_animations(&mut self) {
        let finished = self.animations.finish_inactive();

        for animation in finished {
            let Some(index) = self.window_index_for_id(animation.window_id) else {
                continue;
            };
            let app_id = self.windows[index].window.app_id.clone();
            self.windows[index].window.animating = false;

            match animation.kind {
                AnimationKind::OpenFromDock => {
                    self.windows[index].window.minimized = false;
                    self.windows[index].window.animation_client_pixels = None;
                    println!("Open animation finished: {}", animation.window_id);
                }
                AnimationKind::CloseToDock => {
                    self.windows[index].window.minimized = true;
                    self.windows[index].window.animation_client_pixels = None;
                    self.dock.set_active_for_app(app_id.as_deref(), false);
                    println!("Close animation finished: {}", animation.window_id);
                    self.windows[index].surface.send_close();
                }
                AnimationKind::MinimizeToDock => {
                    self.windows[index].window.minimized = true;
                    self.windows[index].window.animation_client_pixels = None;
                    self.dock.set_active_for_app(app_id.as_deref(), true);
                    println!("Minimize animation finished: {}", animation.window_id);
                }
                AnimationKind::RestoreFromDock => {
                    self.windows[index].window.minimized = false;
                    self.windows[index].window.animation_client_pixels = None;
                    self.dock.set_active_for_app(app_id.as_deref(), false);
                    println!("Restore animation finished: {}", animation.window_id);
                    self.focus_window(animation.window_id);
                }
                AnimationKind::MaximizeWindow => {
                    self.windows[index].window.geometry = animation.to_rect;
                    self.windows[index].window.maximized = true;
                    self.windows[index].window.user_resized = true;
                    println!("Maximize animation finished: {}", animation.window_id);
                    self.configure_window_client_size(animation.window_id);
                }
                AnimationKind::RestoreWindow => {
                    self.windows[index].window.geometry = animation.to_rect;
                    self.windows[index].window.maximized = false;
                    self.windows[index].window.restore_geometry = None;
                    self.windows[index].window.user_resized = true;
                    println!("Window restore animation finished: {}", animation.window_id);
                    self.configure_window_client_size(animation.window_id);
                }
            }

            self.request_redraw();
        }
    }

    fn toggle_maximize_window(&mut self, window_id: WindowId) {
        let Some(index) = self.window_index_for_id(window_id) else {
            return;
        };
        let Some(output) = self.outputs.first().cloned() else {
            return;
        };
        if self.windows[index].window.animating {
            return;
        }

        let decoration = self.window_decoration();
        let from_rect = self.geometry_for_output(self.windows[index].window.geometry);
        let (kind, to_rect) = if self.windows[index].window.maximized {
            (
                AnimationKind::RestoreWindow,
                self.windows[index]
                    .window
                    .restore_geometry
                    .unwrap_or(self.windows[index].window.geometry),
            )
        } else {
            self.windows[index].window.restore_geometry = Some(self.windows[index].window.geometry);
            (
                AnimationKind::MaximizeWindow,
                WindowGeometry {
                    x: 0,
                    y: 0,
                    width: output.width as i32,
                    height: output.height as i32,
                },
            )
        };

        if !self.animations_enabled() {
            self.windows[index].window.geometry = to_rect;
            self.windows[index].window.maximized = matches!(kind, AnimationKind::MaximizeWindow);
            if matches!(kind, AnimationKind::RestoreWindow) {
                self.windows[index].window.restore_geometry = None;
            }
            self.windows[index].window.user_resized = true;
            self.configure_window_client_size(window_id);
            self.request_redraw();
            return;
        }

        self.windows[index].window.animating = true;
        self.animations.start_window_animation(
            kind,
            window_id,
            from_rect,
            to_rect,
            u64::from(self.animations_config.window_open_ms),
            Easing::EaseInOutCubic,
            decoration.corner_radius,
            decoration.corner_radius,
        );

        match kind {
            AnimationKind::MaximizeWindow => println!("Maximize animation started: {window_id}"),
            AnimationKind::RestoreWindow => {
                println!("Window restore animation started: {window_id}");
            }
            _ => {}
        }
        self.request_redraw();
    }

    fn start_window_interaction(&mut self, window_id: WindowId) -> bool {
        let Some(index) = self.window_index_for_id(window_id) else {
            return false;
        };
        if self.windows[index].window.animating || self.windows[index].window.minimized {
            return false;
        }
        let geometry = self.geometry_for_output(self.windows[index].window.geometry);
        let start_cursor = (self.cursor.x, self.cursor.y);
        let edge = self.resize_edge_for_window(window_id, self.cursor.x, self.cursor.y);

        if let Some(edge) = edge {
            self.interaction = PointerInteraction::Resize {
                window_id,
                start_cursor,
                start_geometry: geometry,
                edge,
            };
            return true;
        }

        if !self.is_titlebar_hit(geometry, self.cursor.x, self.cursor.y) {
            return false;
        }
        if self.windows[index].window.maximized {
            return false;
        }

        self.interaction = PointerInteraction::Drag {
            window_id,
            start_cursor,
            start_geometry: geometry,
        };

        true
    }

    fn is_titlebar_hit(&self, geometry: WindowGeometry, x: f64, y: f64) -> bool {
        let decoration = self.window_decoration();

        x >= f64::from(geometry.x)
            && y >= f64::from(geometry.y)
            && x < f64::from(geometry.x + geometry.width)
            && y < f64::from(geometry.y + decoration.titlebar_height)
    }

    fn is_window_interaction_active(&self) -> bool {
        self.interaction != PointerInteraction::None
    }

    fn is_resizing_window(&self, window_id: WindowId) -> bool {
        matches!(
            self.interaction,
            PointerInteraction::Resize {
                window_id: active_window_id,
                ..
            } if active_window_id == window_id
        )
    }

    fn update_window_interaction(&mut self) {
        let interaction = self.interaction;

        match interaction {
            PointerInteraction::None => {}
            PointerInteraction::Drag {
                window_id,
                start_cursor,
                start_geometry,
            } => {
                let dx = (self.cursor.x - start_cursor.0) as i32;
                let dy = (self.cursor.y - start_cursor.1) as i32;
                let output = self.outputs.first().cloned();

                if let Some(window) = self.window_mut_for_id(window_id) {
                    window.geometry.x = start_geometry.x + dx;
                    window.geometry.y = start_geometry.y + dy;
                    if let Some(output) = output.as_ref() {
                        clamp_window_to_output(&mut window.geometry, output);
                    }
                    self.request_redraw();
                }
            }
            PointerInteraction::Resize {
                window_id,
                start_cursor,
                start_geometry,
                edge,
            } => {
                let dx = (self.cursor.x - start_cursor.0) as i32;
                let dy = (self.cursor.y - start_cursor.1) as i32;
                let output = self.outputs.first().cloned();
                let mut resized = false;

                if let Some(window) = self.window_mut_for_id(window_id) {
                    let previous_geometry = window.geometry;
                    window.user_resized = true;
                    match edge {
                        ResizeEdge::Right => {
                            window.geometry.width = (start_geometry.width + dx).max(100);
                        }
                        ResizeEdge::Bottom => {
                            window.geometry.height = (start_geometry.height + dy).max(80);
                        }
                        ResizeEdge::BottomRight => {
                            window.geometry.width = (start_geometry.width + dx).max(100);
                            window.geometry.height = (start_geometry.height + dy).max(80);
                        }
                    }
                    if let Some(output) = output.as_ref() {
                        clamp_window_to_output(&mut window.geometry, output);
                    }
                    resized = window.geometry != previous_geometry;
                    self.request_redraw();
                }

                if resized && self.behavior.windows.send_configure_during_live_resize {
                    self.configure_window_client_size(window_id);
                }
            }
        }
    }

    fn send_pointer_motion(&mut self, window_id: Option<WindowId>) {
        let focus = window_id.and_then(|id| {
            let tracked = self.window_for_id(id)?;
            let geometry = self.geometry_for_output(tracked.window.geometry);
            let decoration = self.window_decoration();
            let (client_x, client_y) = decoration.client_origin(geometry);
            Some((
                tracked.surface.wl_surface().clone(),
                Point::<f64, Logical>::from((f64::from(client_x), f64::from(client_y))),
            ))
        });
        let event = MotionEvent {
            location: Point::from((self.cursor.x, self.cursor.y)),
            serial: SERIAL_COUNTER.next_serial(),
            time: self.frame_timestamp(),
        };
        let pointer = self.pointer.clone();

        pointer.motion(self, focus, &event);
        pointer.frame(self);
    }

    fn send_pointer_button(&mut self, button: WinitMouseButton, pressed: bool) {
        let button = match button {
            WinitMouseButton::Left => 0x110,
            WinitMouseButton::Right => 0x111,
            WinitMouseButton::Middle => 0x112,
        };
        let state = if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };
        let event = ButtonEvent {
            serial: SERIAL_COUNTER.next_serial(),
            time: self.frame_timestamp(),
            button,
            state,
        };
        let pointer = self.pointer.clone();

        pointer.button(self, &event);
        pointer.frame(self);
    }

    fn set_keyboard_focus(&mut self, window_id: WindowId) {
        let Some(surface) = self
            .window_for_id(window_id)
            .map(|tracked| tracked.surface.wl_surface().clone())
        else {
            return;
        };
        let client = surface.client();
        let keyboard = self.keyboard.clone();

        keyboard.set_focus(self, Some(surface), SERIAL_COUNTER.next_serial());
        set_data_device_focus(&self.display_handle, &self.seat, client);
    }

    fn clear_keyboard_focus(&mut self) {
        let keyboard = self.keyboard.clone();

        keyboard.set_focus(self, None, SERIAL_COUNTER.next_serial());
        set_data_device_focus(&self.display_handle, &self.seat, None);
    }

    fn send_keyboard_key(&mut self, keycode: u32, pressed: bool) {
        let state = if pressed {
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        let keyboard = self.keyboard.clone();
        let keycode = Keycode::new(keycode + 8);
        let serial = SERIAL_COUNTER.next_serial();
        let time = self.frame_timestamp();

        let _ = keyboard.input(self, keycode, state, serial, time, |_, _, _| {
            FilterResult::<()>::Forward
        });
    }

    fn window_for_id(&self, window_id: WindowId) -> Option<&TrackedWindow> {
        self.windows
            .iter()
            .find(|tracked| tracked.window.id == window_id)
    }

    fn window_mut_for_id(&mut self, window_id: WindowId) -> Option<&mut Window> {
        self.windows
            .iter_mut()
            .find(|tracked| tracked.window.id == window_id)
            .map(|tracked| &mut tracked.window)
    }

    fn window_index_for_id(&self, window_id: WindowId) -> Option<usize> {
        self.windows
            .iter()
            .position(|tracked| tracked.window.id == window_id)
    }

    fn create_window_scene_node(&mut self, surface: &WlSurface) {
        let Some(output_node) = self.output_scene_nodes.first().copied() else {
            return;
        };
        let Some(index) = self.window_index_for_wl_surface(surface) else {
            return;
        };

        if self.windows[index].scene_node.is_some() {
            return;
        }

        let window = &self.windows[index].window;
        let window_node = self
            .scene
            .add_window(output_node, window.id, window.geometry);
        let _surface_node = self.scene.add_surface(window_node, window.geometry);

        self.windows[index].scene_node = Some(window_node);
        self.request_redraw();
    }

    fn remove_window_scene_node(&mut self, surface: &WlSurface) {
        let Some(index) = self.window_index_for_wl_surface(surface) else {
            return;
        };
        let Some(scene_node_id) = self.windows[index].scene_node.take() else {
            return;
        };

        if self.scene.remove_node(scene_node_id).is_some() {
            println!("Scene node removed");
            self.request_redraw();
        }
    }

    fn window_for_surface_mut(&mut self, surface: &ToplevelSurface) -> Option<&mut TrackedWindow> {
        self.windows
            .iter_mut()
            .find(|tracked| tracked.surface == *surface)
    }

    fn window_index_for_wl_surface(&self, surface: &WlSurface) -> Option<usize> {
        self.windows
            .iter()
            .position(|tracked| tracked.surface.wl_surface() == surface)
    }

    fn surface_metadata(surface: &ToplevelSurface) -> (Option<String>, Option<String>) {
        smithay_compositor::with_states(surface.wl_surface(), |states| {
            let attributes = states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .expect("xdg toplevel surface data should exist")
                .lock()
                .expect("xdg toplevel surface data should not be poisoned");

            (attributes.title.clone(), attributes.app_id.clone())
        })
    }
}

fn create_wayland_output(display_handle: &DisplayHandle, output: &Output) -> SmithayOutput {
    let wayland_output = SmithayOutput::new(
        output.name.clone(),
        PhysicalProperties {
            size: (340, 190).into(),
            subpixel: Subpixel::Unknown,
            make: "LIME".into(),
            model: "Virtual Output".into(),
        },
    );
    let _global = wayland_output.create_global::<CompositorState>(display_handle);
    update_wayland_output(&wayland_output, output);

    wayland_output
}

fn update_wayland_output(wayland_output: &SmithayOutput, output: &Output) {
    let mode = SmithayMode {
        size: (output.width as i32, output.height as i32).into(),
        refresh: output.refresh_rate as i32,
    };

    wayland_output.change_current_state(
        Some(mode),
        Some(smithay::utils::Transform::Normal),
        Some(Scale::Integer(output.scale.round() as i32)),
        Some((0, 0).into()),
    );
    wayland_output.set_preferred(mode);
}

fn clamp_window_to_output(window: &mut WindowGeometry, output: &Output) {
    let output_width = output.width as i32;
    let output_height = output.height as i32;

    if output_width <= 0 || output_height <= 0 {
        return;
    }

    window.width = window.width.max(1).min(output_width);
    window.height = window.height.max(1).min(output_height);
    window.x = window.x.clamp(0, (output_width - window.width).max(0));
    window.y = window.y.clamp(0, (output_height - window.height).max(0));
}

fn button_hit(button: WindowButtonGeometry, x: f64, y: f64) -> bool {
    let radius = f64::from(button.diameter) / 2.0;
    let center_x = f64::from(button.x) + radius;
    let center_y = f64::from(button.y) + radius;
    let dx = x - center_x;
    let dy = y - center_y;

    dx * dx + dy * dy <= radius * radius
}

#[derive(Debug)]
struct TrackedWindow {
    surface: ToplevelSurface,
    window: Window,
    scene_node: Option<SceneNodeId>,
    frame_callbacks: Vec<wl_callback::WlCallback>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SurfaceCommit {
    buffer: Option<CommittedBuffer>,
    frame_callbacks: Vec<wl_callback::WlCallback>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommittedBuffer {
    Attached {
        size: Option<(i32, i32)>,
        pixels: Option<ClientBufferPixels>,
    },
    Removed,
}

impl CommittedBuffer {
    fn is_attached(&self) -> bool {
        matches!(self, Self::Attached { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PointerInteraction {
    None,
    Drag {
        window_id: WindowId,
        start_cursor: (f64, f64),
        start_geometry: WindowGeometry,
    },
    Resize {
        window_id: WindowId,
        start_cursor: (f64, f64),
        start_geometry: WindowGeometry,
        edge: ResizeEdge,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeEdge {
    Right,
    Bottom,
    BottomRight,
}

impl CompositorHandler for CompositorState {
    fn compositor_state(&mut self) -> &mut SmithayCompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<ClientState>()
            .expect("Wayland client data should be initialized by LIME DE")
            .compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        self.update_window_mapping_from_commit(surface);
    }
}

impl XdgShellHandler for CompositorState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        println!("New xdg toplevel surface created");
        let window_id = self.create_window(surface.clone());

        println!("Window created: {window_id}");

        let size = self.window_for_id(window_id).map(|tracked| {
            Size::<i32, Logical>::from(self.client_size_for_geometry(tracked.window.geometry))
        });
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Activated);
            state.size = size;
        });
        surface.send_configure();
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        let _ = surface.send_configure();
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        if let Some(window_id) = self.update_window_title(&surface) {
            println!("Window title changed: {window_id}");
            self.request_redraw();
        }
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        if let Some(window_id) = self.update_window_app_id(&surface) {
            println!("Window app_id changed: {window_id}");
        }
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        if let Some(window_id) = self.remove_window(&surface) {
            println!("Window destroyed: {window_id}");
        }
    }
}

impl BufferHandler for CompositorState {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for CompositorState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SelectionHandler for CompositorState {
    type SelectionUserData = ();
}

impl DataDeviceHandler for CompositorState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for CompositorState {}

impl ServerDndGrabHandler for CompositorState {
    fn send(&mut self, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>) {}
}

impl OutputHandler for CompositorState {}

impl SeatHandler for CompositorState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&Self::KeyboardFocus>) {}

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: CursorImageStatus) {}
}

#[derive(Debug, Default)]
struct ClientState {
    compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}

    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

delegate_compositor!(CompositorState);
delegate_data_device!(CompositorState);
delegate_output!(CompositorState);
delegate_seat!(CompositorState);
delegate_shm!(CompositorState);
delegate_xdg_shell!(CompositorState);

#[derive(Debug)]
struct WaylandRuntime {
    display: Display<CompositorState>,
    event_loop: EventLoop<'static, CompositorState>,
    state: CompositorState,
    socket_name: String,
}

impl Compositor {
    #[must_use]
    pub fn new(
        use_winit_test_backend: bool,
        launch_test_client: bool,
        test_client_commands: Vec<String>,
        style: StyleConfig,
        behavior: BehaviorConfig,
        animations_config: AnimationConfig,
    ) -> Self {
        Self {
            initialized: false,
            running: Arc::new(AtomicBool::new(false)),
            use_winit_test_backend,
            launch_test_client,
            test_client_commands,
            style,
            behavior,
            animations_config,
            test_client: None,
            winit_backend: None,
            wayland: None,
        }
    }

    pub fn initialize(&mut self) -> Result<(), AppError> {
        let display = Display::new()
            .map_err(|error| AppError::new(format!("failed to create Wayland display: {error}")))?;
        let display_handle = display.handle();
        let event_loop = EventLoop::try_new()
            .map_err(|error| AppError::new(format!("failed to create event loop: {error}")))?;
        let loop_signal = event_loop.get_signal();
        let listening_socket = ListeningSocketSource::new_auto()
            .map_err(|error| AppError::new(format!("failed to create Wayland socket: {error}")))?;
        let socket_name = listening_socket
            .socket_name()
            .to_string_lossy()
            .into_owned();

        event_loop
            .handle()
            .insert_source(
                listening_socket,
                |client_stream, _, state: &mut CompositorState| {
                    if let Err(error) = state.insert_client(client_stream) {
                        eprintln!("LIME DE failed to accept Wayland client: {error}");
                    }
                },
            )
            .map_err(|error| {
                AppError::new(format!("failed to register Wayland socket: {error}"))
            })?;

        let state = CompositorState::new(
            display_handle,
            self.style.clone(),
            self.behavior.clone(),
            self.animations_config.clone(),
        );

        if self.use_winit_test_backend {
            let (width, height) = state.primary_output_size();
            self.winit_backend = Some(WinitBackend::new(width, height)?);
        }

        self.wayland = Some(WaylandRuntime {
            display,
            event_loop,
            state,
            socket_name,
        });
        env::set_var("WAYLAND_DISPLAY", &self.socket_name()?);
        self.running.store(true, Ordering::Release);

        let running = Arc::clone(&self.running);
        ctrlc::set_handler(move || {
            if running.swap(false, Ordering::AcqRel) {
                println!("Shutdown requested");
                loop_signal.stop();
            }
        })
        .map_err(|error| AppError::new(format!("failed to install Ctrl+C handler: {error}")))?;

        self.initialized = true;

        println!("Wayland display initialized");
        println!(
            "Wayland socket created: {}",
            self.wayland
                .as_ref()
                .ok_or_else(|| AppError::new("Wayland runtime is missing"))?
                .socket_name
        );
        println!(
            "WAYLAND_DISPLAY={}",
            self.wayland
                .as_ref()
                .ok_or_else(|| AppError::new("Wayland runtime is missing"))?
                .socket_name
        );
        if self.launch_test_client {
            self.launch_test_client();
        }

        Ok(())
    }

    fn socket_name(&self) -> Result<String, AppError> {
        self.wayland
            .as_ref()
            .map(|wayland| wayland.socket_name.clone())
            .ok_or_else(|| AppError::new("Wayland runtime is missing"))
    }

    fn launch_test_client(&mut self) {
        let commands = self.test_client_commands.clone();
        self.launch_client_commands(&commands, true);
    }

    fn launch_client_commands(&mut self, commands: &[String], store_as_test_client: bool) {
        let socket_name = match self.socket_name() {
            Ok(socket_name) => socket_name,
            Err(error) => {
                eprintln!("LIME DE could not launch client: {error}");
                return;
            }
        };

        for command_spec in commands {
            let Some((program, arguments)) = split_command_spec(command_spec) else {
                continue;
            };

            match Command::new(program)
                .args(arguments)
                .env("WAYLAND_DISPLAY", &socket_name)
                .env("XDG_CURRENT_DESKTOP", "LIME")
                .env_remove("DISPLAY")
                .env_remove("DESKTOP_STARTUP_ID")
                .spawn()
            {
                Ok(child) => {
                    if store_as_test_client {
                        self.test_client = Some(child);
                        println!("Test client launched");
                    } else {
                        println!("Dock client launched: {command_spec}");
                    }
                    return;
                }
                Err(_error) => {}
            }
        }

        eprintln!("No dock/test client could be launched");
    }

    pub fn run(&mut self) -> Result<(), AppError> {
        if !self.initialized {
            return Err(AppError::new("compositor runtime is not initialized"));
        }

        println!(
            "Outputs active: {}",
            self.wayland
                .as_ref()
                .ok_or_else(|| AppError::new("Wayland runtime is missing"))?
                .state
                .output_count()
        );
        println!("Event loop running");

        while self.running.load(Ordering::Acquire) {
            let pending_launch_commands = {
                let wayland = self
                    .wayland
                    .as_mut()
                    .ok_or_else(|| AppError::new("Wayland runtime is missing"))?;

                wayland
                    .event_loop
                    .dispatch(Duration::from_millis(16), &mut wayland.state)
                    .map_err(|error| {
                        AppError::new(format!("event loop dispatch failed: {error}"))
                    })?;
                wayland
                    .display
                    .dispatch_clients(&mut wayland.state)
                    .map_err(|error| {
                        AppError::new(format!("Wayland client dispatch failed: {error}"))
                    })?;
                wayland.display.flush_clients().map_err(|error| {
                    AppError::new(format!("Wayland client flush failed: {error}"))
                })?;

                if let Some(winit_backend) = self.winit_backend.as_ref() {
                    if let Some((width, height)) = winit_backend.current_size() {
                        wayland.state.sync_primary_output_size(width, height);
                    }
                    for event in winit_backend.poll_events() {
                        wayland.state.handle_backend_event(event);
                    }
                    if let Some((width, height)) = winit_backend.current_size() {
                        wayland.state.sync_primary_output_size(width, height);
                    }
                }

                wayland.state.recover_stalled_frame_presentation();
                if self.running.load(Ordering::Acquire)
                    && wayland.state.needs_redraw()
                    && wayland.state.should_render_frame()
                {
                    let frame = wayland.state.render_frame();
                    if let Some(winit_backend) = self.winit_backend.as_ref() {
                        if winit_backend.draw_frame(frame) {
                            wayland.state.mark_frame_submitted();
                        }
                    }
                }

                wayland.state.log_memory_debug_tick();
                wayland.state.take_pending_launch_commands()
            };

            for commands in pending_launch_commands {
                self.launch_client_commands(&commands, false);
            }
        }

        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<(), AppError> {
        if self.initialized {
            println!("Compositor runtime shutting down");
        }

        self.running.store(false, Ordering::Release);
        if let Some(mut test_client) = self.test_client.take() {
            match test_client.try_wait() {
                Ok(Some(_status)) => {}
                Ok(None) => {
                    let _ = test_client.kill();
                    let _ = test_client.wait();
                }
                Err(error) => {
                    eprintln!("LIME DE could not query test client state: {error}");
                }
            }
        }
        if let Some(mut winit_backend) = self.winit_backend.take() {
            winit_backend.shutdown();
        }
        self.wayland = None;
        self.initialized = false;

        Ok(())
    }
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new(
            false,
            false,
            Vec::new(),
            StyleConfig::default(),
            BehaviorConfig::default(),
            AnimationConfig::default(),
        )
    }
}

fn split_command_spec(command_spec: &str) -> Option<(String, Vec<String>)> {
    let mut parts = shlex::split(command_spec)?.into_iter();
    let program = parts.next()?;

    Some((program, parts.collect()))
}
