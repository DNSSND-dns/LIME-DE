use std::fmt;

use crate::window::{WindowGeometry, WindowId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(u64);

impl AnimationId {
    #[must_use]
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl fmt::Display for AnimationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationKind {
    OpenFromDock,
    CloseToDock,
    MinimizeToDock,
    RestoreFromDock,
    MaximizeWindow,
    RestoreWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    EaseOutCubic,
    EaseInOutCubic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimatedWindowFrame {
    pub kind: AnimationKind,
    pub rect: WindowGeometry,
    pub radius: i32,
    pub finished: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowAnimation {
    pub id: AnimationId,
    pub kind: AnimationKind,
    pub window_id: WindowId,
    pub from_rect: WindowGeometry,
    pub to_rect: WindowGeometry,
    pub progress: f32,
    pub duration_ms: u64,
    pub easing: Easing,
    pub active: bool,
    pub from_radius: i32,
    pub to_radius: i32,
    elapsed_ms: u64,
}

impl WindowAnimation {
    #[must_use]
    pub fn new(
        id: AnimationId,
        kind: AnimationKind,
        window_id: WindowId,
        from_rect: WindowGeometry,
        to_rect: WindowGeometry,
        duration_ms: u64,
        easing: Easing,
        from_radius: i32,
        to_radius: i32,
    ) -> Self {
        Self {
            id,
            kind,
            window_id,
            from_rect,
            to_rect,
            progress: 0.0,
            duration_ms: duration_ms.max(1),
            easing,
            active: true,
            from_radius,
            to_radius,
            elapsed_ms: 0,
        }
    }

    pub fn update(&mut self, delta_ms: u64) {
        if !self.active {
            return;
        }

        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        self.progress = (self.elapsed_ms as f32 / self.duration_ms as f32).min(1.0);
        if self.progress >= 1.0 {
            self.active = false;
        }
    }

    #[must_use]
    pub fn frame(&self) -> AnimatedWindowFrame {
        let eased = match self.easing {
            Easing::EaseOutCubic => ease_out_cubic(self.progress),
            Easing::EaseInOutCubic => ease_in_out_cubic(self.progress),
        };

        AnimatedWindowFrame {
            kind: self.kind,
            rect: WindowGeometry {
                x: lerp_i32(self.from_rect.x, self.to_rect.x, eased),
                y: lerp_i32(self.from_rect.y, self.to_rect.y, eased),
                width: lerp_i32(self.from_rect.width, self.to_rect.width, eased).max(1),
                height: lerp_i32(self.from_rect.height, self.to_rect.height, eased).max(1),
            },
            radius: lerp_i32(self.from_radius, self.to_radius, eased).max(0),
            finished: !self.active,
        }
    }
}

#[derive(Debug, Default)]
pub struct AnimationManager {
    next_animation_id: u64,
    window_animations: Vec<WindowAnimation>,
}

impl AnimationManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_animation_id: 1,
            window_animations: Vec::new(),
        }
    }

    pub fn start_window_animation(
        &mut self,
        kind: AnimationKind,
        window_id: WindowId,
        from_rect: WindowGeometry,
        to_rect: WindowGeometry,
        duration_ms: u64,
        easing: Easing,
        from_radius: i32,
        to_radius: i32,
    ) {
        self.window_animations
            .retain(|animation| animation.window_id != window_id);
        let id = AnimationId::new(self.next_animation_id);
        self.next_animation_id += 1;
        self.window_animations.push(WindowAnimation::new(
            id,
            kind,
            window_id,
            from_rect,
            to_rect,
            duration_ms,
            easing,
            from_radius,
            to_radius,
        ));
    }

    pub fn update(&mut self, delta_ms: u64) {
        for animation in &mut self.window_animations {
            animation.update(delta_ms);
        }
    }

    pub fn finish_inactive(&mut self) -> Vec<WindowAnimation> {
        let mut finished = Vec::new();
        let mut active = Vec::new();

        for animation in self.window_animations.drain(..) {
            if animation.active {
                active.push(animation);
            } else {
                finished.push(animation);
            }
        }

        self.window_animations = active;
        finished
    }

    #[must_use]
    pub fn frame_for_window(&self, window_id: WindowId) -> Option<AnimatedWindowFrame> {
        self.window_animations
            .iter()
            .find(|animation| animation.window_id == window_id)
            .map(WindowAnimation::frame)
    }

    #[must_use]
    pub fn has_active(&self) -> bool {
        self.window_animations
            .iter()
            .any(|animation| animation.active)
    }
}

#[must_use]
pub fn ease_out_cubic(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    1.0 - (1.0 - progress).powi(3)
}

#[must_use]
pub fn ease_in_out_cubic(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    if progress < 0.5 {
        4.0 * progress * progress * progress
    } else {
        1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0
    }
}

fn lerp_i32(from: i32, to: i32, progress: f32) -> i32 {
    (from as f32 + (to - from) as f32 * progress).round() as i32
}
