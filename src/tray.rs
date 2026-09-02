use std::cell::Cell;

use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};

use crate::icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    OpenControl,
    ToggleTimer,
    Reset,
    ToggleOverlay,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayTimerState {
    Running,
    PausedOvertime,
    PausedReady,
    PausedDisabled,
}

pub struct TrayController {
    _icon: TrayIcon,
    tray_id: tray_icon::TrayIconId,
    open_id: MenuId,
    toggle_timer_id: MenuId,
    reset_id: MenuId,
    toggle_overlay_id: MenuId,
    exit_id: MenuId,
    toggle_timer_item: MenuItem,
    toggle_overlay_item: CheckMenuItem,
    last_timer_state: Cell<Option<TrayTimerState>>,
    last_overlay_visible: Cell<Option<bool>>,
}

impl TrayController {
    pub fn new() -> Result<Self, String> {
        let open_item = MenuItem::with_id("open", "Steuerung öffnen", true, None);
        let toggle_timer_item = MenuItem::with_id("toggle-timer", "Timer starten", true, None);
        let reset_item = MenuItem::with_id("reset", "Timer zurücksetzen", true, None);
        let toggle_overlay_item =
            CheckMenuItem::with_id("toggle-overlay", "Overlay anzeigen", true, true, None);
        let separator = PredefinedMenuItem::separator();
        let exit_item = MenuItem::with_id("exit", "Beenden", true, None);

        let menu_items: [&dyn IsMenuItem; 6] = [
            &open_item,
            &toggle_timer_item,
            &reset_item,
            &toggle_overlay_item,
            &separator,
            &exit_item,
        ];
        let menu = Menu::with_items(&menu_items)
            .map_err(|error| format!("Tray-Menü konnte nicht erstellt werden: {error}"))?;
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_menu_on_right_click(true)
            .with_tooltip("Overlay Timer")
            .with_icon(
                icon::tray_icon().map_err(|error| format!("Tray-Icon ist ungültig: {error}"))?,
            )
            .build()
            .map_err(|error| format!("Tray-Icon konnte nicht erstellt werden: {error}"))?;

        Ok(Self {
            tray_id: tray_icon.id().clone(),
            open_id: open_item.id().clone(),
            toggle_timer_id: toggle_timer_item.id().clone(),
            reset_id: reset_item.id().clone(),
            toggle_overlay_id: toggle_overlay_item.id().clone(),
            exit_id: exit_item.id().clone(),
            _icon: tray_icon,
            toggle_timer_item,
            toggle_overlay_item,
            last_timer_state: Cell::new(None),
            last_overlay_visible: Cell::new(None),
        })
    }

    pub fn sync(
        &self,
        timer_running: bool,
        timer_overtime: bool,
        timer_action_enabled: bool,
        overlay_visible: bool,
    ) {
        let timer_state = if timer_running {
            TrayTimerState::Running
        } else if timer_overtime {
            TrayTimerState::PausedOvertime
        } else if timer_action_enabled {
            TrayTimerState::PausedReady
        } else {
            TrayTimerState::PausedDisabled
        };
        if self.last_timer_state.replace(Some(timer_state)) != Some(timer_state) {
            let (label, enabled) = match timer_state {
                TrayTimerState::Running => ("Timer pausieren", true),
                TrayTimerState::PausedOvertime => ("Timer fortsetzen", true),
                TrayTimerState::PausedReady => ("Timer starten", true),
                TrayTimerState::PausedDisabled => ("Timer starten", false),
            };
            self.toggle_timer_item.set_text(label);
            self.toggle_timer_item.set_enabled(enabled);
        }
        if self.last_overlay_visible.replace(Some(overlay_visible)) != Some(overlay_visible) {
            self.toggle_overlay_item.set_checked(overlay_visible);
        }
    }

    pub fn drain_commands(&self) -> Vec<TrayCommand> {
        let mut commands = Vec::new();

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let command = if event.id == self.open_id {
                Some(TrayCommand::OpenControl)
            } else if event.id == self.toggle_timer_id {
                Some(TrayCommand::ToggleTimer)
            } else if event.id == self.reset_id {
                Some(TrayCommand::Reset)
            } else if event.id == self.toggle_overlay_id {
                Some(TrayCommand::ToggleOverlay)
            } else if event.id == self.exit_id {
                Some(TrayCommand::Exit)
            } else {
                None
            };
            commands.extend(command);
        }

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if event.id() != &self.tray_id {
                continue;
            }
            let should_open = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            );
            if should_open {
                commands.push(TrayCommand::OpenControl);
            }
        }

        commands
    }
}
