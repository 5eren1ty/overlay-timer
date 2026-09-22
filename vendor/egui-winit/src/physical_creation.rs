//! Local extension for exact physical geometry before creating a native window.
//! All other attributes and post-creation operations retain upstream behavior.
use egui::{Context, Id, ViewportBuilder};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    window::WindowAttributes,
};

/// Geometry in physical desktop pixels. Size is the client size.
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub position: PhysicalPosition<i32>,
    pub size: PhysicalSize<u32>,
}

fn key(title: &str) -> Id {
    Id::new(("egui-winit physical creation geometry", title))
}

/// Set or clear creation-only geometry for an exact window title in this context.
/// Does not move, resize, recreate, show, or change the styles of an existing window.
pub fn set(ctx: &Context, title: &str, geometry: Option<Geometry>) {
    ctx.data_mut(|data| {
        if let Some(geometry) = geometry {
            data.insert_temp(key(title), geometry);
        } else {
            data.remove::<Geometry>(key(title));
        }
    });
}

/// Prepare native creation attributes and the remaining post-creation settings.
/// Geometry overrides apply only to ordinary windows explicitly registered by title.
pub fn prepare(ctx: &Context, builder: &ViewportBuilder) -> (WindowAttributes, ViewportBuilder) {
    let mut attributes = super::create_winit_window_attributes(ctx, builder.clone());
    let mut after_creation = builder.clone();
    if builder.monitor.is_none()
        && builder.fullscreen != Some(true)
        && let Some(geometry) = ctx.data(|data| data.get_temp::<Geometry>(key(&attributes.title)))
    {
        attributes = attributes
            .with_position(geometry.position)
            .with_inner_size(geometry.size);
        // Do not reapply the logical fallback after the HWND was created.
        // Passthrough and all unrelated settings are still applied as before.
        after_creation.position = None;
        after_creation.inner_size = None;
    }
    (attributes, after_creation)
}
