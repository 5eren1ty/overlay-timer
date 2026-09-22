use std::sync::{
    Arc, RwLock,
    mpsc::{self, Receiver, Sender},
};

use eframe::egui::{self, Align2, Color32, RichText};

use crate::meme::{MemeOverlay, MemePlayback};

const OVERLAY_VIEWPORT_ID: &str = "overlay_timer_viewport";
const CARD_ANIMATION_ID: &str = "overlay_timer_card_animation";
const CARD_ANIMATION_SECONDS: f64 = 0.24;
const CARD_HORIZONTAL_PADDING: f32 = 24.0;
const CARD_VERTICAL_PADDING: f32 = 14.0;

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

#[derive(Debug, Clone, Copy, PartialEq)]
enum CardPlacement {
    Anchored { anchor: Align2, offset: egui::Vec2 },
    Custom([f32; 2]),
}

impl CardPlacement {
    fn from_snapshot(snapshot: &OverlaySnapshot) -> Self {
        snapshot.custom_position.map_or(
            Self::Anchored {
                anchor: snapshot.anchor,
                offset: snapshot.anchor_offset,
            },
            Self::Custom,
        )
    }

    fn initial_rect(self, size: egui::Vec2, screen: egui::Rect) -> egui::Rect {
        match self {
            Self::Anchored { anchor, offset } => anchor
                .align_size_within_rect(size, screen)
                .translate(offset),
            Self::Custom(position) => {
                egui::Rect::from_center_size(position_from_normalized(position, screen), size)
            }
        }
    }

    fn expansion_bias(self) -> egui::Vec2 {
        match self {
            Self::Anchored { anchor, .. } => {
                egui::vec2(anchor.x().to_factor(), anchor.y().to_factor())
            }
            Self::Custom([x, y]) => egui::vec2(x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)),
        }
    }
}

#[derive(Debug, Clone)]
struct PreviousCardText {
    text: String,
    font_size: f32,
    color: Color32,
}

#[derive(Debug, Clone)]
struct CardAnimation {
    placement: CardPlacement,
    screen: egui::Rect,
    logical_target: egui::Rect,
    from: egui::Rect,
    to: egui::Rect,
    started_at: f64,
    current_text: String,
    current_font_size: f32,
    current_color: Color32,
    previous_text: Option<PreviousCardText>,
}

#[derive(Debug, Clone)]
struct AnimatedCardFrame {
    rect: egui::Rect,
    transition_progress: f32,
    previous_text: Option<PreviousCardText>,
    animating: bool,
}

impl CardAnimation {
    fn new(
        placement: CardPlacement,
        screen: egui::Rect,
        size: egui::Vec2,
        text: String,
        font_size: f32,
        color: Color32,
        now: f64,
    ) -> Self {
        let logical_target = placement.initial_rect(size, screen);
        let target = fit_card_rect(logical_target, screen);
        Self {
            placement,
            screen,
            logical_target,
            from: target,
            to: target,
            started_at: now,
            current_text: text,
            current_font_size: font_size,
            current_color: color,
            previous_text: None,
        }
    }

    fn frame(
        &mut self,
        desired_size: egui::Vec2,
        text: &str,
        font_size: f32,
        color: Color32,
        now: f64,
        animate: bool,
    ) -> AnimatedCardFrame {
        self.finish_completed_animation(now);

        if !animate && self.is_animating(now) {
            self.from = self.to;
            self.started_at = now;
            self.previous_text = None;
        }

        if !approximately_equal(self.logical_target.size(), desired_size) {
            let current = self.current_rect(now);
            self.logical_target = resize_card_rect(
                self.logical_target,
                desired_size,
                self.placement.expansion_bias(),
            );
            let target = fit_card_rect(self.logical_target, self.screen);

            if animate {
                self.from = current;
                self.to = target;
                self.started_at = now;
                self.previous_text = Some(PreviousCardText {
                    text: self.current_text.clone(),
                    font_size: self.current_font_size,
                    color: self.current_color,
                });
            } else {
                self.from = target;
                self.to = target;
                self.started_at = now;
                self.previous_text = None;
            }
        } else if self.current_text != text {
            // Equal-width timer ticks should remain crisp. Only a geometry
            // change needs a cross-fade and a moving card.
            self.previous_text = None;
        }

        self.current_text.clear();
        self.current_text.push_str(text);
        self.current_font_size = font_size;
        self.current_color = color;

        let raw_progress = self.raw_progress(now);
        let transition_progress = ease_in_out_cubic(raw_progress);
        let animating = raw_progress < 1.0 && self.from != self.to;
        let rect = lerp_rect(self.from, self.to, transition_progress);
        if !animating {
            self.from = self.to;
            self.previous_text = None;
        }

        AnimatedCardFrame {
            rect,
            transition_progress,
            previous_text: self.previous_text.clone(),
            animating,
        }
    }

    fn raw_progress(&self, now: f64) -> f32 {
        ((now - self.started_at) / CARD_ANIMATION_SECONDS).clamp(0.0, 1.0) as f32
    }

    fn current_rect(&self, now: f64) -> egui::Rect {
        lerp_rect(
            self.from,
            self.to,
            ease_in_out_cubic(self.raw_progress(now)),
        )
    }

    fn is_animating(&self, now: f64) -> bool {
        self.from != self.to && self.raw_progress(now) < 1.0
    }

    fn finish_completed_animation(&mut self, now: f64) {
        if self.from != self.to && self.raw_progress(now) >= 1.0 {
            self.from = self.to;
            self.previous_text = None;
        }
    }
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
        self.sync_native(root_ui.ctx());
        // Step 2: a valid target geometry is enough to create the overlay
        // visible. Do not wait for an HWND that can only exist after creation.
        // Retain the native error guard for an existing, invalid window.
        let viewport = inset_viewport_builder(
            snapshot.visible && geometry.is_some() && self.native_error().is_none(),
            snapshot.edit_mode,
        );
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
                    clear_card_animation(overlay_ui.ctx());
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

                let color = if snapshot.overtime {
                    Color32::from_rgb(255, 105, 105)
                } else {
                    Color32::from_rgb(
                        snapshot.text_color[0],
                        snapshot.text_color[1],
                        snapshot.text_color[2],
                    )
                };
                let galley = timer_text_galley(overlay_ui, &snapshot.time, snapshot.font_size);
                let desired_size = galley.size()
                    + egui::vec2(CARD_HORIZONTAL_PADDING * 2.0, CARD_VERTICAL_PADDING * 2.0);
                let now = overlay_ui.input(|input| input.time);
                let animated = animated_card_frame(
                    overlay_ui.ctx(),
                    CardPlacement::from_snapshot(&snapshot),
                    screen_rect,
                    desired_size,
                    &snapshot.time,
                    snapshot.font_size,
                    color,
                    now,
                    !snapshot.edit_mode,
                );

                // Position by the already measured top-left corner. Unlike an
                // anchored Area this never needs the previous frame's size.
                let area = egui::Area::new(egui::Id::new("countdown"))
                    .movable(false)
                    .interactable(snapshot.edit_mode)
                    .constrain(false)
                    .fade_in(false)
                    .pivot(Align2::LEFT_TOP)
                    .current_pos(animated.rect.min);

                let timer_card = area.show(overlay_ui.ctx(), |ui| {
                    let (card_rect, card_response) =
                        ui.allocate_exact_size(animated.rect.size(), egui::Sense::hover());
                    ui.painter().rect_filled(
                        card_rect,
                        12.0,
                        Color32::from_black_alpha(snapshot.background_opacity),
                    );

                    let painter = ui
                        .painter()
                        .with_clip_rect(card_rect.intersect(screen_rect));
                    if let Some(previous) = &animated.previous_text {
                        let previous_galley =
                            timer_text_galley(ui, &previous.text, previous.font_size);
                        paint_card_text(
                            &painter,
                            card_rect,
                            previous_galley,
                            previous
                                .color
                                .gamma_multiply(1.0 - animated.transition_progress),
                        );
                    }
                    let current_opacity = if animated.previous_text.is_some() {
                        animated.transition_progress
                    } else {
                        1.0
                    };
                    paint_card_text(
                        &painter,
                        card_rect,
                        galley,
                        color.gamma_multiply(current_opacity),
                    );

                    if snapshot.edit_mode {
                        edit_card_transform(
                            ui,
                            card_response,
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

fn timer_text_galley(ui: &egui::Ui, text: &str, font_size: f32) -> Arc<egui::Galley> {
    ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::FontId::monospace(font_size),
        Color32::PLACEHOLDER,
    )
}

fn paint_card_text(
    painter: &egui::Painter,
    card_rect: egui::Rect,
    galley: Arc<egui::Galley>,
    color: Color32,
) {
    let position = card_rect.center() - galley.size() * 0.5;
    painter.galley(position, galley, color);
}

#[expect(clippy::too_many_arguments)]
fn animated_card_frame(
    ctx: &egui::Context,
    placement: CardPlacement,
    screen: egui::Rect,
    desired_size: egui::Vec2,
    text: &str,
    font_size: f32,
    color: Color32,
    now: f64,
    animate: bool,
) -> AnimatedCardFrame {
    let id = egui::Id::new(CARD_ANIMATION_ID);
    let mut frame = None;
    ctx.data_mut(|data| {
        let mut state = data
            .get_temp::<CardAnimation>(id)
            .filter(|state| state.placement == placement && state.screen == screen)
            .unwrap_or_else(|| {
                CardAnimation::new(
                    placement,
                    screen,
                    desired_size,
                    text.to_owned(),
                    font_size,
                    color,
                    now,
                )
            });
        frame = Some(state.frame(desired_size, text, font_size, color, now, animate));
        data.insert_temp(id, state);
    });

    let frame = frame.expect("the card frame is assigned while egui data is locked");
    if frame.animating {
        // The normal timer cadence is 100 ms. Ask the overlay viewport for
        // immediate frames only while a 240 ms card transition is active.
        ctx.request_repaint_of(OverlayBridge::viewport_id());
    }
    frame
}

fn clear_card_animation(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<CardAnimation>(egui::Id::new(CARD_ANIMATION_ID)));
}

fn resize_card_rect(rect: egui::Rect, desired_size: egui::Vec2, bias: egui::Vec2) -> egui::Rect {
    let delta = desired_size - rect.size();
    egui::Rect::from_min_max(
        rect.min - egui::vec2(bias.x * delta.x, bias.y * delta.y),
        rect.max + egui::vec2((1.0 - bias.x) * delta.x, (1.0 - bias.y) * delta.y),
    )
}

fn fit_card_rect(rect: egui::Rect, screen: egui::Rect) -> egui::Rect {
    let horizontal_shift = if rect.width() >= screen.width() {
        screen.center().x - rect.center().x
    } else if rect.left() < screen.left() {
        screen.left() - rect.left()
    } else if rect.right() > screen.right() {
        screen.right() - rect.right()
    } else {
        0.0
    };
    let vertical_shift = if rect.height() >= screen.height() {
        screen.center().y - rect.center().y
    } else if rect.top() < screen.top() {
        screen.top() - rect.top()
    } else if rect.bottom() > screen.bottom() {
        screen.bottom() - rect.bottom()
    } else {
        0.0
    };
    rect.translate(egui::vec2(horizontal_shift, vertical_shift))
}

fn lerp_rect(from: egui::Rect, to: egui::Rect, progress: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(
            egui::lerp(from.min.x..=to.min.x, progress),
            egui::lerp(from.min.y..=to.min.y, progress),
        ),
        egui::pos2(
            egui::lerp(from.max.x..=to.max.x, progress),
            egui::lerp(from.max.y..=to.max.y, progress),
        ),
    )
}

fn ease_in_out_cubic(progress: f32) -> f32 {
    if progress < 0.5 {
        4.0 * progress.powi(3)
    } else {
        1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0
    }
}

fn approximately_equal(left: egui::Vec2, right: egui::Vec2) -> bool {
    (left.x - right.x).abs() <= 0.01 && (left.y - right.y).abs() <= 0.01
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
    const HORIZONTAL_MARGIN: f32 = CARD_HORIZONTAL_PADDING * 2.0;
    const VERTICAL_MARGIN: f32 = CARD_VERTICAL_PADDING * 2.0;
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
            for visible in [false, true] {
                let builder = inset_viewport_builder(visible, false);
                let (attributes, after) = egui_winit::physical_creation::prepare(&ctx, &builder);
                assert_eq!(attributes.position, Some(Position::Physical(position)));
                assert_eq!(attributes.inner_size, Some(Size::Physical(size)));
                assert_eq!(attributes.visible, visible);
                assert!(!attributes.active);
                assert!(attributes.transparent);
                assert!(attributes.fullscreen.is_none());
                assert!(after.inner_size.is_none());
                assert!(after.position.is_none());
                assert_eq!(after.mouse_passthrough, Some(true));
                assert_eq!(after.has_shadow, Some(false));
            }
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

    #[test]
    fn card_growth_is_distributed_by_its_normalized_position() {
        let start = egui::Rect::from_min_size(egui::pos2(300.0, 200.0), egui::vec2(180.0, 80.0));
        let desired_size = egui::vec2(240.0, 80.0);

        let one_third = resize_card_rect(start, desired_size, egui::vec2(1.0 / 3.0, 0.5));
        assert_rect_close(
            one_third,
            egui::Rect::from_min_max(egui::pos2(280.0, 200.0), egui::pos2(520.0, 280.0)),
        );

        let centered = resize_card_rect(start, desired_size, egui::Vec2::splat(0.5));
        assert_rect_close(
            centered,
            egui::Rect::from_min_max(egui::pos2(270.0, 200.0), egui::pos2(510.0, 280.0)),
        );

        let right = resize_card_rect(start, desired_size, egui::vec2(1.0, 0.5));
        assert_rect_close(
            right,
            egui::Rect::from_min_max(egui::pos2(240.0, 200.0), egui::pos2(480.0, 280.0)),
        );
    }

    #[test]
    fn overtime_prefix_produces_the_expected_geometry_change() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let remaining = timer_text_galley(ui, "00:00", 64.0);
            let overtime = timer_text_galley(ui, "+00:00", 64.0);

            assert!(overtime.size().x > remaining.size().x);
            assert!((overtime.size().y - remaining.size().y).abs() < 0.001);
            assert!(
                overtime
                    .job
                    .sections
                    .iter()
                    .all(|section| { section.format.color == Color32::PLACEHOLDER })
            );
        });
        output.textures_delta.clear();
    }

    #[test]
    fn growing_and_shrinking_with_the_same_bias_is_reversible() {
        let start = egui::Rect::from_min_size(egui::pos2(300.0, 200.0), egui::vec2(180.0, 80.0));
        for bias in [0.0, 1.0 / 3.0, 0.5, 1.0] {
            let grown = resize_card_rect(start, egui::vec2(240.0, 80.0), egui::vec2(bias, 0.5));
            let restored = resize_card_rect(grown, start.size(), egui::vec2(bias, 0.5));
            assert_rect_close(restored, start);
        }
    }

    #[test]
    fn fitted_animation_stays_inside_every_horizontal_screen_position() {
        let screen =
            egui::Rect::from_min_size(egui::pos2(-1_920.0, 120.0), egui::vec2(1_920.0, 1_080.0));
        for x in [0.0, 0.1, 1.0 / 3.0, 0.5, 0.9, 1.0] {
            let placement = CardPlacement::Custom([x, 0.5]);
            let mut animation = CardAnimation::new(
                placement,
                screen,
                egui::vec2(180.0, 80.0),
                "00:00".into(),
                64.0,
                Color32::WHITE,
                0.0,
            );

            for step in 0..=24 {
                let now = f64::from(step) * CARD_ANIMATION_SECONDS / 24.0;
                let frame = animation.frame(
                    egui::vec2(240.0, 80.0),
                    "+00:00",
                    64.0,
                    Color32::RED,
                    now,
                    true,
                );
                assert!(screen.expand(0.001).contains_rect(frame.rect));
            }
        }
    }

    #[test]
    fn geometry_changes_animate_but_equal_width_ticks_do_not() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
        let mut animation = CardAnimation::new(
            CardPlacement::Custom([1.0 / 3.0, 0.5]),
            screen,
            egui::vec2(180.0, 80.0),
            "00:00".into(),
            64.0,
            Color32::WHITE,
            0.0,
        );

        let tick = animation.frame(
            egui::vec2(180.0, 80.0),
            "00:59",
            64.0,
            Color32::WHITE,
            0.0,
            true,
        );
        assert!(!tick.animating);
        assert!(tick.previous_text.is_none());

        let start = animation.frame(
            egui::vec2(240.0, 80.0),
            "+00:00",
            64.0,
            Color32::RED,
            1.0,
            true,
        );
        assert!(start.animating);
        assert_eq!(start.previous_text.as_ref().unwrap().text, "00:59");

        let middle = animation.frame(
            egui::vec2(240.0, 80.0),
            "+00:00",
            64.0,
            Color32::RED,
            1.0 + CARD_ANIMATION_SECONDS / 2.0,
            true,
        );
        assert!(middle.animating);
        assert!(middle.rect.width() > start.rect.width());
        assert!(middle.rect.width() < 240.0);

        let end = animation.frame(
            egui::vec2(240.0, 80.0),
            "+00:00",
            64.0,
            Color32::RED,
            1.0 + CARD_ANIMATION_SECONDS,
            true,
        );
        assert!(!end.animating);
        assert!(end.previous_text.is_none());
        assert!((end.rect.width() - 240.0).abs() < 0.001);
    }

    fn assert_rect_close(actual: egui::Rect, expected: egui::Rect) {
        assert!((actual.min.x - expected.min.x).abs() < 0.001);
        assert!((actual.min.y - expected.min.y).abs() < 0.001);
        assert!((actual.max.x - expected.max.x).abs() < 0.001);
        assert!((actual.max.y - expected.max.y).abs() < 0.001);
    }
}
