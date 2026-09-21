# Local patch: explicit Windows shadow at window creation

Upstream: egui-winit 0.36.1, egui commit
4c1f2fae95475a40e524884ebb298bcb1714b08e, crates/egui-winit.

Only the Windows window-attribute construction in src/lib.rs is changed:
ViewportBuilder.has_shadow, when specified, overrides the default
!decorations value for with_undecorated_shadow. An absent value retains upstream
behavior. Cargo.toml only adjusts packaged license paths to this directory.

The application keeps decorations(false) and has_shadow(false) constant for the
inset overlay. Visibility and mouse-passthrough commands preserve winit's stored
shadow flag. A future explicit Decorations command still follows upstream's
automatic shadow behavior: this is not a complete new cross-platform API.
The application tests ensure its edit/visibility transitions emit no such
command. A has_shadow change recreates a viewport through upstream egui logic.

No WM_NCCALCSIZE subclass, no renderer changes and no fullscreen shadow removal
are part of this patch. The inset overlay starts hidden and is positioned and
verified before it is shown.

Upstream licenses are included unchanged.
