# Local egui-winit changes

Upstream: egui-winit 0.36.1, egui commit
4c1f2fae95475a40e524884ebb298bcb1714b08e, crates/egui-winit.
Upstream licenses are included unchanged; Cargo.toml adjusts their local paths.

## Existing Windows shadow option

The Windows attribute construction honors ViewportBuilder.has_shadow when set,
otherwise retaining upstream !decorations behavior. Runtime Decorations commands
still follow upstream shadow behavior. The application keeps decorations(false)
and has_shadow(false) constant.

## Initial physical geometry experiment

The local physical_creation module allows an application to register physical
client size and outer position by exact window title within its egui Context.
create_window uses these attributes before calling event_loop.create_window.
It then omits only position/inner_size from the post-creation builder application,
so the logical fallback cannot resize the new window before surface creation.
Mouse passthrough is still applied in its original position in the sequence.

Unregistered titles, fullscreen/monitor-selected windows, and existing windows
retain their original behavior. Clearing the registration removes the override.
The application registers only its normal inset overlay.

No renderer, visibility, activation, shadow timing or passthrough timing changes
are part of this experiment. Native correction for subsequent monitor changes
is retained and any actual correction is logged explicitly.
