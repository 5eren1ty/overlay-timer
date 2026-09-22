use std::{
    collections::hash_map::RandomState,
    hash::BuildHasher,
    io::Cursor,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use eframe::egui::{self, Color32};
use image::AnimationDecoder;

use crate::timer::TimerReading;

// Bundled into the executable so the GIF also works outside the project folder.
const MEME_GIF: &[u8] = include_bytes!("../hurry-up-judge-judy.gif");

pub fn growth_interval_seconds(configured: u64) -> u64 {
    // The bundled GIF takes 1.8 seconds, leaving a gap even at the minimum.
    configured.clamp(2, 60)
}

fn elapsed_in_stage(elapsed: Duration, interval: u64) -> Duration {
    Duration::new(
        elapsed.as_secs() % growth_interval_seconds(interval),
        elapsed.subsec_nanos(),
    )
}

#[derive(Debug, Clone, Copy)]
pub struct MemePlayback {
    elapsed: Duration,
    sampled_at: Instant,
    running: bool,
}

impl MemePlayback {
    pub fn new(enabled: bool, reading: TimerReading, running: bool, now: Instant) -> Option<Self> {
        match (enabled, reading) {
            (true, TimerReading::Overtime(elapsed)) => Some(Self {
                elapsed,
                sampled_at: now,
                running,
            }),
            _ => None,
        }
    }

    fn elapsed(self, now: Instant) -> Duration {
        if self.running {
            self.elapsed
                .saturating_add(now.saturating_duration_since(self.sampled_at))
        } else {
            self.elapsed
        }
    }
}

#[derive(Default)]
pub struct MemeOverlay {
    // Decode once, only when first needed; share frames between viewport callbacks.
    animation: OnceLock<Result<Mutex<GifAnimation>, String>>,
    placement: Mutex<Placement>,
}

impl MemeOverlay {
    pub fn deactivate(&self) {
        self.placement
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .key = None;
    }

    pub fn error(&self) -> Option<&str> {
        self.animation
            .get()
            .and_then(|result| result.as_ref().err())
            .map(String::as_str)
    }

    pub fn paint(
        &self,
        ctx: &egui::Context,
        playback: MemePlayback,
        interval: u64,
        screen: egui::Rect,
        timer: egui::Rect,
    ) {
        let animation = self
            .animation
            .get_or_init(|| GifAnimation::decode(MEME_GIF).map(Mutex::new));
        let Ok(animation) = animation else { return };
        let mut animation = animation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let elapsed = playback.elapsed(Instant::now());
        let stage_elapsed = elapsed_in_stage(elapsed, interval);
        let Some(index) = animation.frame_index(stage_elapsed) else {
            if playback.running {
                ctx.request_repaint_after(
                    Duration::from_secs(growth_interval_seconds(interval)) - stage_elapsed,
                );
            }
            // A complete pass is over: paint nothing until the next stage,
            // keeping the old position so its replacement can move elsewhere.
            return;
        };
        if animation.current_frame != Some(index) {
            let image = animation.frames[index].image.clone();
            if let Some(texture) = animation.texture.as_mut() {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                animation.texture =
                    Some(ctx.load_texture("meme_gif", image, egui::TextureOptions::LINEAR));
            }
            animation.current_frame = Some(index);
        }
        if let Some(texture) = &animation.texture {
            let size = meme_size(animation.size, screen.size(), elapsed, interval);
            let rect = self
                .placement
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .rect(size, screen, timer, elapsed, interval);
            // Keep the timer card and edit controls above the meme, with no hit area.
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Background,
                egui::Id::new("meme_overlay"),
            ))
            .with_clip_rect(screen)
            .image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        if playback.running {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}

#[derive(Default)]
struct Placement {
    key: Option<(u64, u64, egui::Rect, egui::Vec2)>,
    previous: Option<egui::Rect>,
    timer: Option<egui::Rect>,
    elapsed: Duration,
}

impl Placement {
    fn rect(
        &mut self,
        size: egui::Vec2,
        screen: egui::Rect,
        timer: egui::Rect,
        elapsed: Duration,
        interval: u64,
    ) -> egui::Rect {
        let interval = growth_interval_seconds(interval);
        let key = (elapsed.as_secs() / interval, interval, screen, size);
        let obstacle = timer.expand(16.0);
        let timer_moved_into_meme = self.timer != Some(timer)
            && self
                .previous
                .is_some_and(|rect| overlap_area(rect, obstacle) > 0.0);
        if self.key != Some(key) || elapsed < self.elapsed || timer_moved_into_meme {
            self.previous = Some(choose_position(size, screen, obstacle, self.previous));
            self.key = Some(key);
        }
        self.timer = Some(timer);
        self.elapsed = elapsed;
        self.previous
            .expect("A position is assigned before painting")
    }
}

fn overlap_area(a: egui::Rect, b: egui::Rect) -> f32 {
    let intersection = a.intersect(b);
    intersection.width().max(0.0) * intersection.height().max(0.0)
}

fn choose_position(
    size: egui::Vec2,
    screen: egui::Rect,
    obstacle: egui::Rect,
    previous: Option<egui::Rect>,
) -> egui::Rect {
    // Every valid center lies in at least one of these four strips. Sampling
    // those explicitly also finds narrow free spaces that rejection sampling misses.
    let centers = screen.shrink2(size * 0.5);
    let forbidden = obstacle.expand2(size * 0.5);
    let regions = [
        egui::Rect::from_min_max(
            centers.min,
            egui::pos2(centers.max.x.min(forbidden.min.x), centers.max.y),
        ),
        egui::Rect::from_min_max(
            egui::pos2(centers.min.x.max(forbidden.max.x), centers.min.y),
            centers.max,
        ),
        egui::Rect::from_min_max(
            centers.min,
            egui::pos2(centers.max.x, centers.max.y.min(forbidden.min.y)),
        ),
        egui::Rect::from_min_max(
            egui::pos2(centers.min.x, centers.min.y.max(forbidden.max.y)),
            centers.max,
        ),
        // If the GIF cannot fit beside the timer, minimize overlap at screen edges.
        centers,
    ];
    let random = RandomState::new();
    let mut counter = 0_u64;
    let mut next = || {
        counter += 1;
        random.hash_one(counter)
    };
    let minimum_move = (screen.size().min_elem() * 0.15).min(160.0);
    let mut best: Option<(egui::Rect, f32, f32, u64)> = None;
    for region in regions {
        if region.width() < 0.0 || region.height() < 0.0 {
            continue;
        }
        for sample in 0..12 {
            let center = match sample {
                0 => region.left_top(),
                1 => region.right_top(),
                2 => region.left_bottom(),
                3 => region.right_bottom(),
                _ => {
                    let x = (next() >> 40) as f32 / 16_777_216.0;
                    let y = (next() >> 40) as f32 / 16_777_216.0;
                    region.min + region.size() * egui::vec2(x, y)
                }
            };
            let rect = egui::Rect::from_center_size(center, size);
            let overlap = overlap_area(rect, obstacle);
            let too_close = previous.map_or(0.0, |last| {
                (minimum_move - last.center().distance(center)).max(0.0)
            });
            let rank = next();
            if best
                .as_ref()
                .is_none_or(|&(_, old_overlap, old_close, old_rank)| {
                    (overlap, too_close, rank) < (old_overlap, old_close, old_rank)
                })
            {
                best = Some((rect, overlap, too_close, rank));
            }
        }
    }
    best.map_or_else(
        || egui::Rect::from_center_size(screen.center(), size),
        |(rect, _, _, _)| rect,
    )
}

struct GifFrame {
    image: egui::ColorImage,
    ends_at: Duration,
}

struct GifAnimation {
    frames: Vec<GifFrame>,
    size: egui::Vec2,
    duration: Duration,
    texture: Option<egui::TextureHandle>,
    current_frame: Option<usize>,
}

impl GifAnimation {
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
            .map_err(|error| error.to_string())?;
        let mut frames = Vec::new();
        let mut duration = Duration::ZERO;
        for frame in decoder.into_frames() {
            // The decoder composites partial frames and applies GIF disposal rules.
            let frame = frame.map_err(|error| error.to_string())?;
            let (numerator, denominator) = frame.delay().numer_denom_ms();
            let milliseconds = (f64::from(numerator) / f64::from(denominator.max(1))).max(10.0);
            duration += Duration::from_secs_f64(milliseconds / 1000.0);
            let buffer = frame.into_buffer();
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [buffer.width() as usize, buffer.height() as usize],
                buffer.as_raw(),
            );
            frames.push(GifFrame {
                image,
                ends_at: duration,
            });
        }
        let first = frames.first().ok_or("Das GIF enthält keine Bilder.")?;
        let size = egui::vec2(first.image.size[0] as f32, first.image.size[1] as f32);
        Ok(Self {
            frames,
            size,
            duration,
            texture: None,
            current_frame: None,
        })
    }

    fn frame_index(&self, elapsed: Duration) -> Option<usize> {
        // GIF loop metadata is deliberately ignored: one pass per stage.
        (elapsed < self.duration).then(|| {
            self.frames
                .partition_point(|frame| frame.ends_at <= elapsed)
        })
    }
}

fn meme_size(
    source: egui::Vec2,
    screen: egui::Vec2,
    elapsed: Duration,
    interval: u64,
) -> egui::Vec2 {
    let max_scale = (screen.x * 0.7 / source.x).min(screen.y * 0.7 / source.y);
    let initial_scale = (240.0 / source.x).min(max_scale);
    let steps = elapsed.as_secs() / growth_interval_seconds(interval);
    let scale = (initial_scale * (1.0 + steps as f32 * 0.25)).min(max_scale);
    source * scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::CountdownTimer;

    #[test]
    fn bundled_gif_plays_one_complete_pass_without_looping() {
        let animation = GifAnimation::decode(MEME_GIF).unwrap();
        assert!(animation.frames.len() > 1);
        assert!(animation.size.x > 0.0 && animation.size.y > 0.0);
        assert_eq!(animation.frame_index(Duration::ZERO), Some(0));
        assert_eq!(animation.frame_index(animation.frames[0].ends_at), Some(1));
        assert_eq!(animation.duration, Duration::from_millis(1_800));
        assert_eq!(
            animation.frame_index(animation.duration - Duration::from_nanos(1)),
            Some(animation.frames.len() - 1)
        );
        assert_eq!(animation.frame_index(animation.duration), None);
        assert_eq!(animation.frame_index(animation.duration * 2), None);
        assert!(
            animation
                .frames
                .windows(2)
                .any(|pair| pair[0].image.pixels != pair[1].image.pixels)
        );
    }

    #[test]
    fn growth_obeys_interval_preserves_aspect_ratio_and_stays_on_screen() {
        let source = egui::vec2(400.0, 300.0);
        let screen = egui::vec2(1920.0, 1080.0);
        assert!(
            (meme_size(source, screen, Duration::from_secs(4), 5) - egui::vec2(240.0, 180.0))
                .length()
                < 0.001
        );
        assert!(
            (meme_size(source, screen, Duration::from_secs(5), 5) - egui::vec2(300.0, 225.0))
                .length()
                < 0.001
        );
        for screen in [screen, egui::vec2(160.0, 90.0), egui::vec2(800.0, 1200.0)] {
            for elapsed in [Duration::ZERO, Duration::from_secs(u64::MAX)] {
                let size = meme_size(source, screen, elapsed, 0);
                assert!(size.x <= screen.x * 0.7 + 0.01);
                assert!(size.y <= screen.y * 0.7 + 0.01);
                assert!((size.x / size.y - 4.0 / 3.0).abs() < 0.001);
            }
        }
    }

    #[test]
    fn playback_follows_expiry_pause_resume_reset_and_mode_switch() {
        let start = Instant::now();
        let mut timer = CountdownTimer::new(Duration::from_secs(10));
        let snapshot = |timer: &CountdownTimer, enabled, now| {
            MemePlayback::new(enabled, timer.reading(now), timer.is_running(), now)
        };
        assert!(snapshot(&timer, true, start).is_none());
        timer.toggle(start);
        assert!(snapshot(&timer, true, start + Duration::from_secs(9)).is_none());
        let expired = start + Duration::from_secs(10);
        assert_eq!(
            snapshot(&timer, true, expired).unwrap().elapsed(expired),
            Duration::ZERO
        );
        assert_eq!(
            snapshot(&timer, true, expired)
                .unwrap()
                .elapsed(expired + Duration::from_secs(2)),
            Duration::from_secs(2)
        );
        timer.toggle(expired + Duration::from_secs(3));
        let later = expired + Duration::from_secs(20);
        assert_eq!(
            snapshot(&timer, true, later)
                .unwrap()
                .elapsed(later + Duration::from_secs(5)),
            Duration::from_secs(3)
        );
        timer.toggle(later);
        assert_eq!(
            snapshot(&timer, true, later)
                .unwrap()
                .elapsed(later + Duration::from_secs(5)),
            Duration::from_secs(8)
        );
        assert!(snapshot(&timer, false, later).is_none());
        timer.reset();
        assert!(snapshot(&timer, true, later).is_none());
    }

    #[test]
    fn renderer_uploads_animation_frames_and_clears_when_inactive() {
        for interval in [2, 5] {
            let overlay = MemeOverlay::default();
            let ctx = egui::Context::default();
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1920.0, 1080.0));
            let render = |elapsed| {
                ctx.run_ui(egui::RawInput::default(), |ui| {
                    overlay.paint(
                        ui.ctx(),
                        MemePlayback {
                            elapsed,
                            sampled_at: Instant::now(),
                            running: false,
                        },
                        interval,
                        screen,
                        egui::Rect::from_min_size(
                            egui::pos2(1500.0, 900.0),
                            egui::vec2(300.0, 120.0),
                        ),
                    );
                })
            };
            let mut first = render(Duration::ZERO);
            first.textures_delta.clear();
            assert!(overlay.error().is_none());
            let image_bounds = |output: &egui::FullOutput| {
                output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::epaint::Shape::Mesh(mesh)
                            if mesh.texture_id != egui::TextureId::default() =>
                        {
                            Some(mesh.calc_bounds())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            let first_bounds = image_bounds(&first);
            assert_eq!(first_bounds.len(), 1);
            assert!((first_bounds[0].width() - 240.0).abs() < 0.001);
            assert!(screen.contains_rect(first_bounds[0]));
            let next_frame = overlay
                .animation
                .get()
                .unwrap()
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .frames[0]
                .ends_at;
            let mut second = render(next_frame);
            let uploaded_frame = !second.textures_delta.set.is_empty();
            second.textures_delta.clear();
            assert!(uploaded_frame);
            assert_eq!(image_bounds(&second)[0], first_bounds[0]);
            let mut last_frame = render(Duration::from_millis(1_799));
            last_frame.textures_delta.clear();
            assert_eq!(image_bounds(&last_frame), first_bounds);
            for elapsed in [
                Duration::from_millis(1_800),
                Duration::from_secs(interval) - Duration::from_millis(1),
            ] {
                let mut gap = render(elapsed);
                let uploads = gap.textures_delta.set.len();
                gap.textures_delta.clear();
                assert!(image_bounds(&gap).is_empty());
                assert_eq!(uploads, 0);
            }
            let mut grown = render(Duration::from_secs(interval));
            grown.textures_delta.clear();
            assert_eq!(
                overlay
                    .animation
                    .get()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .current_frame,
                Some(0)
            );
            assert!((image_bounds(&grown)[0].width() - 300.0).abs() < 0.001);
            assert_ne!(image_bounds(&grown)[0].center(), first_bounds[0].center());
            overlay.deactivate();
            let mut hidden = ctx.run_ui(egui::RawInput::default(), |_| {});
            hidden.textures_delta.clear();
            assert!(image_bounds(&hidden).is_empty());
        }
    }
    #[test]
    fn placement_avoids_corner_and_freely_positioned_timers_on_offset_monitors() {
        for origin in [egui::Pos2::ZERO, egui::pos2(-1920.0, 120.0)] {
            let screen = egui::Rect::from_min_size(origin, egui::vec2(1920.0, 1080.0));
            for position in [
                egui::vec2(0.0, 0.0),
                egui::vec2(1550.0, 0.0),
                egui::vec2(0.0, 920.0),
                egui::vec2(1550.0, 920.0),
                egui::vec2(800.0, 400.0),
            ] {
                let timer = egui::Rect::from_min_size(origin + position, egui::vec2(370.0, 160.0));
                let mut placement = Placement::default();
                for step in 0..30 {
                    let rect = placement.rect(
                        egui::vec2(300.0, 225.0),
                        screen,
                        timer,
                        Duration::from_secs(step * 5),
                        5,
                    );
                    assert!(screen.expand(0.001).contains_rect(rect));
                    assert!(overlap_area(rect, timer.expand(16.0)) < 0.001);
                }
            }
        }
    }

    #[test]
    fn position_changes_only_at_intervals_or_when_placement_is_invalidated() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1920.0, 1080.0));
        let timer = egui::Rect::from_min_size(egui::pos2(1550.0, 900.0), egui::vec2(300.0, 120.0));
        let size = egui::vec2(300.0, 225.0);
        let mut placement = Placement::default();
        let first = placement.rect(size, screen, timer, Duration::ZERO, 5);
        assert_eq!(
            placement.rect(size, screen, timer, Duration::from_secs(4), 5),
            first
        );
        // Same size also models continued movement after reaching the growth cap.
        let next = placement.rect(size, screen, timer, Duration::from_secs(5), 5);
        assert!(first.center().distance(next.center()) >= 159.0);
        assert_eq!(
            placement.rect(size, screen, timer, Duration::from_secs(5), 5),
            next
        );
        let moved_timer = next;
        let relocated = placement.rect(size, screen, moved_timer, Duration::from_secs(5), 5);
        assert_eq!(overlap_area(relocated, moved_timer.expand(16.0)), 0.0);
        placement.key = None;
        let restarted = placement.rect(size, screen, moved_timer, Duration::ZERO, 5);
        assert!(restarted.center().distance(relocated.center()) >= 159.0);
        let smaller_screen =
            egui::Rect::from_min_size(egui::pos2(-800.0, 50.0), egui::vec2(800.0, 600.0));
        let resized = placement.rect(size, smaller_screen, timer, Duration::ZERO, 5);
        assert!(smaller_screen.expand(0.001).contains_rect(resized));
    }

    #[test]
    fn narrow_free_space_is_found_and_impossible_overlap_is_minimized_without_jitter() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(1000.0));
        let obstacle = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(698.0, 1000.0));
        let rect = choose_position(egui::vec2(300.0, 500.0), screen, obstacle, None);
        assert_eq!(overlap_area(rect, obstacle), 0.0);
        assert!(screen.contains_rect(rect));
        let timer = egui::Rect::from_min_max(egui::pos2(216.0, 216.0), egui::pos2(784.0, 784.0));
        let mut placement = Placement::default();
        let size = egui::Vec2::splat(700.0);
        let first = placement.rect(size, screen, timer, Duration::ZERO, 5);
        assert_eq!(overlap_area(first, timer.expand(16.0)), 250_000.0);
        assert_eq!(
            placement.rect(size, screen, timer, Duration::from_secs(1), 5),
            first
        );
    }
    #[test]
    fn each_stage_plays_one_pass_then_waits_and_restarts() {
        let animation = GifAnimation::decode(MEME_GIF).unwrap();
        for configured in [0, 1, 2, 3, 4, 5, 60, u64::MAX] {
            let interval = growth_interval_seconds(configured);
            assert!((2..=60).contains(&interval));
            let cycle = Duration::from_secs(interval);
            assert!(animation.duration < cycle);
            for stage in [0, 1, 10] {
                let start = cycle * stage;
                assert_eq!(
                    animation.frame_index(elapsed_in_stage(start, configured)),
                    Some(0)
                );
                assert_eq!(
                    animation.frame_index(elapsed_in_stage(
                        start + animation.duration - Duration::from_nanos(1),
                        configured
                    )),
                    Some(animation.frames.len() - 1)
                );
                assert_eq!(
                    animation.frame_index(elapsed_in_stage(start + animation.duration, configured)),
                    None
                );
                assert_eq!(
                    animation.frame_index(elapsed_in_stage(
                        start + cycle - Duration::from_nanos(1),
                        configured
                    )),
                    None
                );
                assert_eq!(
                    animation.frame_index(elapsed_in_stage(start + cycle, configured)),
                    Some(0)
                );
            }
        }
    }

    #[test]
    fn pausing_during_the_shortest_gap_preserves_the_remaining_wait() {
        let animation = GifAnimation::decode(MEME_GIF).unwrap();
        let start = Instant::now();
        let mut timer = CountdownTimer::new(Duration::from_secs(1));
        timer.toggle(start);
        timer.toggle(start + Duration::from_millis(2_900));
        let later = start + Duration::from_secs(100);
        let playback =
            MemePlayback::new(true, timer.reading(later), timer.is_running(), later).unwrap();
        assert_eq!(
            elapsed_in_stage(playback.elapsed(later + Duration::from_secs(50)), 2),
            Duration::from_millis(1_900)
        );
        timer.toggle(later);
        let resumed =
            MemePlayback::new(true, timer.reading(later), timer.is_running(), later).unwrap();
        assert_eq!(
            animation.frame_index(elapsed_in_stage(
                resumed.elapsed(later + Duration::from_millis(99)),
                2
            )),
            None
        );
        assert_eq!(
            animation.frame_index(elapsed_in_stage(
                resumed.elapsed(later + Duration::from_millis(100)),
                2
            )),
            Some(0)
        );
    }
    #[test]
    fn invalid_gif_returns_an_error() {
        assert!(GifAnimation::decode(b"invalid gif").is_err());
    }
}
