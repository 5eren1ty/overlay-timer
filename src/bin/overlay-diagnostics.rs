#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../diagnostics/mod.rs"]
mod diagnostics;

fn main() -> eframe::Result {
    diagnostics::run()
}
