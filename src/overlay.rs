use std::sync::{
    Arc, RwLock,
    mpsc::{self, Receiver, Sender},
};

use eframe::egui::{self, Align2, Color32, RichText};

use crate::meme::{MemeOverlay, MemePlayback};

const OVERLAY_VIEWPORT_ID: &str = "overlay_timer_viewport";

#[derive(Debug, Clone)]
pub struct OverlaySnapshot {
    pub visible: bool,
    pub monitor_index: usize,
    pub anchor: Align2,
    pub anchor_offset: egui::Vec2,
    pub custom_position: Option<[f32; 2]>,
    pub font_size: f32,
    pub text_color: [u8; 3],
    pub background_opacity: u8,
    pub edit_mode: bool,
    pub time: String,
    pub overtime: bool,
    pub meme: Option<MemePlayback>,
    pub meme_growth_interval_seconds: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum OverlayEvent {
    PositionChanged([f32; 2]),
    TransformChanged { position: [f32; 2], font_size: f32 },
    ExitEditMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeCorner {
    const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    fn position(self, rect: egui::Rect) -> egui::Pos2 {
        match self {
            Self::TopLeft => rect.left_top(),
            Self::TopRight => rect.right_top(),
            Self::BottomLeft => rect.left_bottom(),
            Self::BottomRight => rect.right_bottom(),
        }
    }

    fn opposite_position(self, rect: egui::Rect) -> egui::Pos2 {
        match self {
            Self::TopLeft => rect.right_bottom(),
            Self::TopRight => rect.left_bottom(),
            Self::BottomLeft => rect.right_top(),
            Self::BottomRight => rect.left_top(),
        }
    }

    fn cursor(self) -> egui::CursorIcon {
        match self {
            Self::TopLeft | Self::BottomRight => egui::CursorIcon::ResizeNwSe,
            Self::TopRight | Self::BottomLeft => egui::CursorIcon::ResizeNeSw,
        }
    }

    fn hit_rect(self, rect: egui::Rect, size: f32) -> egui::Rect {
        let size = egui::Vec2::splat(size);
        match self {
            Self::TopLeft => egui::Rect::from_min_size(rect.left_top(), size),
            Self::TopRight => {
                egui::Rect::from_min_size(rect.right_top() - egui::vec2(size.x, 0.0), size)
            }
            Self::BottomLeft => {
                egui::Rect::from_min_size(rect.left_bottom() - egui::vec2(0.0, size.y), size)
            }
            Self::BottomRight => egui::Rect::from_min_size(rect.right_bottom() - size, size),
        }
    }

    fn at_pointer(rect: egui::Rect, pointer: egui::Pos2, size: f32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|corner| corner.hit_rect(rect, size).contains(pointer))
    }

    fn center_keeping_opposite_fixed(self, opposite: egui::Pos2, size: egui::Vec2) -> egui::Pos2 {
        let half_size = size * 0.5;
        match self {
            Self::TopLeft => opposite - half_size,
            Self::TopRight => opposite + egui::vec2(half_size.x, -half_size.y),
            Self::BottomLeft => opposite + egui::vec2(-half_size.x, half_size.y),
            Self::BottomRight => opposite + half_size,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MoveGesture {
    start_center: egui::Pos2,
}

#[derive(Debug, Clone, Copy)]
struct ResizeGesture {
    corner: ResizeCorner,
    start_rect: egui::Rect,
    start_font_size: f32,
}

pub struct OverlayBridge {
    snapshot: Arc<RwLock<OverlaySnapshot>>,
    events_tx: Sender<OverlayEvent>,
    events_rx: Receiver<OverlayEvent>,
    meme: Arc<MemeOverlay>,
    native_error: Arc<RwLock<Option<String>>>,
}

impl OverlayBridge {
    pub fn new(snapshot: OverlaySnapshot) -> Self {
        let (events_tx, events_rx) = mpsc::channel();
        Self {
            snapshot: Arc::new(RwLock::new(snapshot)),
            events_tx,
            events_rx,
            meme: Arc::new(MemeOverlay::default()),
            native_error: Arc::new(RwLock::new(None)),
        }
    }

    pub fn meme_error(&self) -> Option<&str> {
        self.meme.error()
    }

    pub fn update(&self, snapshot: OverlaySnapshot) {
        if !snapshot.visible || snapshot.meme.is_none() {
            self.meme.deactivate();
        }
        *self
            .snapshot
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = snapshot;
    }

    pub fn drain_events(&self) -> impl Iterator<Item = OverlayEvent> + '_ {
        self.events_rx.try_iter()
    }

    pub fn viewport_id() -> egui::ViewportId {
        egui::ViewportId::from_hash_of(OVERLAY_VIEWPORT_ID)
    }

    pub fn native_error(&self) -> Option<String> {
        self.native_error
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn sync_native(&self, ctx: &egui::Context) -> bool {
        synchronize_native(ctx, &self.snapshot(), &self.native_error)
    }

    pub fn request_repaint(ctx: &egui::Context) {
        ctx.request_repaint_after_for(std::time::Duration::from_millis(100), Self::viewport_id());
    }

    pub fn show(&self, root_ui: &mut egui::Ui) {
        let snapshot = self.snapshot();
        let geometry = crate::windows_overlay::target_rect(snapshot.monitor_index)
            .ok()
            .map(
                |[x, y, width, height]| egui_winit::physical_creation::Geometry {
                    position: winit::dpi::PhysicalPosition::new(x, y),
                    size: winit::dpi::PhysicalSize::new(width as u32, height as u32),
                },
            );
        egui_winit::physical_creation::set(root_ui.ctx(), "Overlay Timer", geometry);
        let ready = self.sync_native(root_ui.ctx());
        let viewport = inset_viewport_builder(snapshot.visible && ready, snapshot.edit_mode);
        let shared_snapshot = Arc::clone(&self.snapshot);
        let events_tx = self.events_tx.clone();
        let meme = Arc::clone(&self.meme);
        let native_error = Arc::clone(&self.native_error);

        root_ui.ctx().show_viewport_deferred(
            Self::viewport_id(),
            viewport,
            move |overlay_ui, viewport_class| {
                if viewport_class == egui::ViewportClass::EmbeddedWindow {
                    return;
                }

                let snapshot = shared_snapshot
                    .read()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone();
                if !snapshot.visible
                    || !synchronize_native(overlay_ui.ctx(), &snapshot, &native_error)
                {
                    return;
                }

                let screen_rect = overlay_ui.ctx().content_rect();

                let escape_pressed = snapshot.edit_mode
                    && overlay_ui.input(|input| input.key_pressed(egui::Key::Escape));
                let exit_button_clicked =
                    snapshot.edit_mode && edit_mode_exit_button(overlay_ui.ctx());
                if escape_pressed || exit_button_clicked {
                    let _ = events_tx.send(OverlayEvent::ExitEditMode);
                    overlay_ui.ctx().send_viewport_cmd_to(
                        Self::viewport_id(),
                        egui::ViewportCommand::MousePassthrough(true),
                    );
                    overlay_ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
                }

                let mut area = egui::Area::new(egui::Id::new("countdown"))
                    .movable(false)
                    .interactable(snapshot.edit_mode)
                    .constrain_to(screen_rect)
                    .fade_in(false);

                if let Some(position) = snapshot.custom_position {
                    area = area
                        .pivot(Align2::CENTER_CENTER)
                        .current_pos(position_from_normalized(position, screen_rect));
                } else if snapshot.edit_mode {
                    area = area.pivot(snapshot.anchor).current_pos(
                        snapshot.anchor.pos_in_rect(&screen_rect) + snapshot.anchor_offset,
                    );
                } else {
                    area = area.anchor(snapshot.anchor, snapshot.anchor_offset);
                }

                let timer_card = area.show(overlay_ui.ctx(), |ui| {
                    let frame_response = egui::Frame::new()
                        .fill(Color32::from_black_alpha(snapshot.background_opacity))
                        .corner_radius(12.0)
                        .inner_margin(egui::Margin::symmetric(24, 14))
                        .show(ui, |ui| {
                            let color = if snapshot.overtime {
                                Color32::from_rgb(255, 105, 105)
                            } else {
                                Color32::from_rgb(
                                    snapshot.text_color[0],
                                    snapshot.text_color[1],
                                    snapshot.text_color[2],
                                )
                            };

                            ui.add(
                                egui::Label::new(
                                    RichText::new(&snapshot.time)
                                        .size(snapshot.font_size)
                                        .color(color)
                                        .strong()
                                        .monospace(),
                                )
                                .extend(),
                            );
                        });

                    let card_rect = frame_response.response.rect;
                    if snapshot.edit_mode {
                        edit_card_transform(
                            ui,
                            frame_response.response,
                            screen_rect,
                            &snapshot,
                            &shared_snapshot,
                            &events_tx,
                        );
                    }
                    card_rect
                });
                if let Some(playback) = snapshot.meme {
                    meme.paint(
                        overlay_ui.ctx(),
                        playback,
                        snapshot.meme_growth_interval_seconds,
                        screen_rect,
                        timer_card.inner,
                    );
                }
            },
        );
    }

    fn snapshot(&self) -> OverlaySnapshot {
        self.snapshot
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

fn inset_viewport_builder(visible: bool, edit_mode: bool) -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title("Overlay Timer")
        .with_decorations(false)
        .with_has_shadow(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_mouse_passthrough(!edit_mode)
        .with_taskbar(false)
        .with_active(false)
        .with_visible(visible)
        .with_resizable(false)
        // Fallback only for an unavailable monitor (the native guard keeps it
        // hidden). For a valid monitor physical_creation overrides these
        // attributes before window creation and skips the later logical resize.
        .with_inner_size([800.0, 450.0])
}

fn synchronize_native(
    ctx: &egui::Context,
    snapshot: &OverlaySnapshot,
    error: &RwLock<Option<String>>,
) -> bool {
    let (ready, current_visible, new_error) =
        match crate::windows_overlay::prepare_overlay(snapshot.monitor_index) {
            Ok(Some(window)) => (true, Some(window.visible), None),
            Ok(None) => (false, None, None),
            Err(message) => (false, Some(true), Some(message)),
        };
    {
        let mut error = error.write().unwrap_or_else(|p| p.into_inner());
        if *error != new_error {
            *error = new_error;
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
    }
    let desired_visible = snapshot.visible && ready;
    if current_visible.is_some_and(|current| current != desired_visible) {
        // Goes through winit so its visibility/style state remains consistent.
        ctx.send_viewport_cmd_to(
            OverlayBridge::viewport_id(),
            egui::ViewportCommand::Visible(desired_visible),
        );
        ctx.request_repaint_of(OverlayBridge::viewport_id());
    }
    ready
}

fn edit_mode_exit_button(ctx: &egui::Context) -> bool {
    egui::Area::new(egui::Id::new("overlay_edit_mode_exit"))
        .anchor(Align2::CENTER_TOP, egui::vec2(0.0, 24.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.add(
                egui::Button::new(
                    RichText::new("Bearbeitung beenden  ·  Esc")
                        .color(Color32::WHITE)
                        .strong(),
                )
                .fill(Color32::from_rgb(37, 99, 235))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(147, 197, 253)))
                .corner_radius(9.0)
                .min_size(egui::vec2(240.0, 40.0)),
            )
            .clicked()
        })
        .inner
}

fn edit_card_transform(
    ui: &mut egui::Ui,
    card_response: egui::Response,
    screen_rect: egui::Rect,
    snapshot: &OverlaySnapshot,
    shared_snapshot: &Arc<RwLock<OverlaySnapshot>>,
    events_tx: &Sender<OverlayEvent>,
) {
    const HANDLE_VISUAL_SIZE: f32 = 10.0;
    const HANDLE_HIT_SIZE: f32 = 28.0;
    const SELECTION_COLOR: Color32 = Color32::from_rgb(80, 170, 255);

    let move_gesture_id = egui::Id::new("overlay_card_move_gesture");
    let resize_gesture_id = egui::Id::new("overlay_card_resize_gesture");
    let card_rect = card_response.rect;
    let transform_response = card_response.interact(egui::Sense::drag());

    if transform_response.drag_started()
        && let Some(pointer) = transform_response.interact_pointer_pos()
    {
        ui.ctx().data_mut(|data| {
            data.remove::<ResizeGesture>(resize_gesture_id);
            data.remove::<MoveGesture>(move_gesture_id);
            if let Some(corner) = ResizeCorner::at_pointer(card_rect, pointer, HANDLE_HIT_SIZE) {
                data.insert_temp(
                    resize_gesture_id,
                    ResizeGesture {
                        corner,
                        start_rect: card_rect,
                        start_font_size: snapshot.font_size,
                    },
                );
            } else {
                data.insert_temp(
                    move_gesture_id,
                    MoveGesture {
                        start_center: card_rect.center(),
                    },
                );
            }
        });
    }

    let resize_gesture = ui
        .ctx()
        .data(|data| data.get_temp::<ResizeGesture>(resize_gesture_id));
    if transform_response.hovered() || transform_response.dragged() {
        if let Some(corner) = resize_gesture.map(|gesture| gesture.corner).or_else(|| {
            ui.ctx()
                .pointer_hover_pos()
                .and_then(|pointer| ResizeCorner::at_pointer(card_rect, pointer, HANDLE_HIT_SIZE))
        }) {
            ui.ctx().set_cursor_icon(corner.cursor());
        } else {
            ui.ctx().set_cursor_icon(if transform_response.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }
    }

    if transform_response.dragged() {
        let drag_delta = transform_response.total_drag_delta().unwrap_or_default();
        if let Some(gesture) = resize_gesture {
            let scale = proportional_scale_from_drag(gesture, drag_delta);
            let font_size = (gesture.start_font_size * scale).clamp(32.0, 128.0);
            let card_size = estimated_card_size(
                gesture.start_rect.size(),
                gesture.start_font_size,
                font_size,
            );
            let opposite = gesture.corner.opposite_position(gesture.start_rect);
            let center = gesture
                .corner
                .center_keeping_opposite_fixed(opposite, card_size);
            let center = clamp_card_center(center, card_size, screen_rect);
            let position = normalized_position(center, screen_rect);

            {
                let mut shared = shared_snapshot
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                shared.font_size = font_size;
                shared.custom_position = Some(position);
            }
            let _ = events_tx.send(OverlayEvent::TransformChanged {
                position,
                font_size,
            });
        } else if let Some(gesture) = ui
            .ctx()
            .data(|data| data.get_temp::<MoveGesture>(move_gesture_id))
        {
            let center = clamp_card_center(
                gesture.start_center + drag_delta,
                card_rect.size(),
                screen_rect,
            );
            let position = normalized_position(center, screen_rect);
            shared_snapshot
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .custom_position = Some(position);
            let _ = events_tx.send(OverlayEvent::PositionChanged(position));
        }
    }

    if transform_response.drag_stopped() {
        ui.ctx().data_mut(|data| {
            data.remove::<MoveGesture>(move_gesture_id);
            data.remove::<ResizeGesture>(resize_gesture_id);
        });
    }

    ui.painter().rect_stroke(
        card_rect.expand(2.0),
        14.0,
        egui::Stroke::new(2.0, SELECTION_COLOR),
        egui::StrokeKind::Outside,
    );

    for corner in ResizeCorner::ALL {
        let visual_rect = egui::Rect::from_center_size(
            corner.position(card_rect),
            egui::Vec2::splat(HANDLE_VISUAL_SIZE),
        );
        ui.painter().rect(
            visual_rect,
            2.0,
            SELECTION_COLOR,
            egui::Stroke::new(1.0, Color32::WHITE),
            egui::StrokeKind::Outside,
        );
    }
}

fn proportional_scale_from_drag(gesture: ResizeGesture, drag_delta: egui::Vec2) -> f32 {
    let fixed = gesture.corner.opposite_position(gesture.start_rect);
    let start = gesture.corner.position(gesture.start_rect) - fixed;
    let current = gesture.corner.position(gesture.start_rect) + drag_delta - fixed;
    let denominator = start.length_sq().max(f32::EPSILON);
    current.dot(start) / denominator
}

fn estimated_card_size(start_size: egui::Vec2, start_font_size: f32, font_size: f32) -> egui::Vec2 {
    const HORIZONTAL_MARGIN: f32 = 48.0;
    const VERTICAL_MARGIN: f32 = 28.0;
    let scale = font_size / start_font_size.max(f32::EPSILON);
    egui::vec2(
        (start_size.x - HORIZONTAL_MARGIN).max(0.0) * scale + HORIZONTAL_MARGIN,
        (start_size.y - VERTICAL_MARGIN).max(0.0) * scale + VERTICAL_MARGIN,
    )
}

fn clamp_card_center(
    center: egui::Pos2,
    card_size: egui::Vec2,
    screen_rect: egui::Rect,
) -> egui::Pos2 {
    let half_size = card_size * 0.5;
    let x = if card_size.x >= screen_rect.width() {
        screen_rect.center().x
    } else {
        center.x.clamp(
            screen_rect.left() + half_size.x,
            screen_rect.right() - half_size.x,
        )
    };
    let y = if card_size.y >= screen_rect.height() {
        screen_rect.center().y
    } else {
        center.y.clamp(
            screen_rect.top() + half_size.y,
            screen_rect.bottom() - half_size.y,
        )
    };
    egui::pos2(x, y)
}

fn normalized_position(position: egui::Pos2, rect: egui::Rect) -> [f32; 2] {
    let width = rect.width().max(1.0);
    let height = rect.height().max(1.0);
    [
        ((position.x - rect.left()) / width).clamp(0.0, 1.0),
        ((position.y - rect.top()) / height).clamp(0.0, 1.0),
    ]
}

fn position_from_normalized(position: [f32; 2], rect: egui::Rect) -> egui::Pos2 {
    egui::pos2(
        rect.left() + position[0].clamp(0.0, 1.0) * rect.width(),
        rect.top() + position[1].clamp(0.0, 1.0) * rect.height(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inset_visibility_and_edit_transitions_keep_shadow_and_mode_constant() {
        let mut builder = inset_viewport_builder(false, false);
        assert_eq!(builder.has_shadow, Some(false));
        assert_eq!(builder.monitor, None);
        assert_eq!(builder.fullscreen, None);
        for (visible, edit) in [(true, false), (true, true), (false, true), (true, false)] {
            let (commands, recreate) = builder.patch(inset_viewport_builder(visible, edit));
            assert!(!recreate);
            assert!(!commands.iter().any(|command| matches!(
                command,
                egui::ViewportCommand::Decorations(_)
                    | egui::ViewportCommand::Fullscreen(_)
                    | egui::ViewportCommand::SetMonitor(_)
            )));
            assert_eq!(builder.has_shadow, Some(false));
        }
    }

    #[test]
    fn creation_geometry_is_physical_and_does_not_reapply_logical_fallback() {
        use winit::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
        for [x, y, width, height] in [[2561, 244, 1918, 1078], [-2559, -1439, 2558, 1438]] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(1.5);
            let position = PhysicalPosition::new(x, y);
            let size = PhysicalSize::new(width as u32, height as u32);
            egui_winit::physical_creation::set(
                &ctx,
                "Overlay Timer",
                Some(egui_winit::physical_creation::Geometry { position, size }),
            );
            let builder = inset_viewport_builder(false, false);
            let (attributes, after) = egui_winit::physical_creation::prepare(&ctx, &builder);
            assert_eq!(attributes.position, Some(Position::Physical(position)));
            assert_eq!(attributes.inner_size, Some(Size::Physical(size)));
            assert!(!attributes.visible);
            assert!(!attributes.active);
            assert!(attributes.transparent);
            assert!(attributes.fullscreen.is_none());
            assert!(after.inner_size.is_none());
            assert!(after.position.is_none());
            assert_eq!(after.mouse_passthrough, builder.mouse_passthrough);
            assert_eq!(after.has_shadow, Some(false));
        }
    }

    #[test]
    fn creation_override_is_scoped_and_can_be_cleared() {
        let ctx = egui::Context::default();
        egui_winit::physical_creation::set(
            &ctx,
            "Overlay Timer",
            Some(egui_winit::physical_creation::Geometry {
                position: winit::dpi::PhysicalPosition::new(1, 1),
                size: winit::dpi::PhysicalSize::new(1918, 1078),
            }),
        );
        for builder in [
            inset_viewport_builder(false, false).with_title("Controller"),
            inset_viewport_builder(false, false).with_fullscreen(true),
            inset_viewport_builder(false, false).with_monitor(0),
        ] {
            let (_, after) = egui_winit::physical_creation::prepare(&ctx, &builder);
            assert_eq!(after.inner_size, Some(egui::vec2(800.0, 450.0)));
        }
        egui_winit::physical_creation::set(&ctx, "Overlay Timer", None);
        let (_, after) =
            egui_winit::physical_creation::prepare(&ctx, &inset_viewport_builder(false, false));
        assert_eq!(after.inner_size, Some(egui::vec2(800.0, 450.0)));
    }

    #[test]
    fn normalized_positions_round_trip_across_an_offset_monitor() {
        let monitor =
            egui::Rect::from_min_size(egui::pos2(-1_920.0, 120.0), egui::vec2(1_920.0, 1_080.0));
        let normalized = [0.73, 0.42];

        let position = position_from_normalized(normalized, monitor);
        let result = normalized_position(position, monitor);

        assert!((result[0] - normalized[0]).abs() < f32::EPSILON);
        assert!((result[1] - normalized[1]).abs() < f32::EPSILON);
    }

    #[test]
    fn normalized_positions_are_clamped_to_the_monitor() {
        let monitor = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));

        assert_eq!(
            normalized_position(egui::pos2(-10.0, 120.0), monitor),
            [0.0, 1.0]
        );
        assert_eq!(
            position_from_normalized([-1.0, 2.0], monitor),
            egui::pos2(0.0, 100.0)
        );
    }

    #[test]
    fn resize_scale_is_based_on_the_fixed_drag_origin() {
        let start_rect = egui::Rect::from_min_size(egui::pos2(20.0, 30.0), egui::vec2(100.0, 50.0));
        let gesture = ResizeGesture {
            corner: ResizeCorner::BottomRight,
            start_rect,
            start_font_size: 64.0,
        };

        let scale = proportional_scale_from_drag(gesture, egui::vec2(100.0, 50.0));

        assert!((scale - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn every_resize_corner_keeps_its_opposite_corner_fixed() {
        let start_rect = egui::Rect::from_min_size(egui::pos2(40.0, 60.0), egui::vec2(148.0, 78.0));
        let resized = estimated_card_size(start_rect.size(), 50.0, 100.0);

        for corner in ResizeCorner::ALL {
            let fixed = corner.opposite_position(start_rect);
            let center = corner.center_keeping_opposite_fixed(fixed, resized);
            let result = egui::Rect::from_center_size(center, resized);

            assert_eq!(corner.opposite_position(result), fixed);
        }
    }
}
