use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::{error::AppError, settings::Settings};

pub const WINDOW_LABEL: &str = "mini";
pub const REFRESH_EVENT: &str = "mini-refresh";

/// Base quota-only dimensions preserved from the original mini bar.
pub const BASE_WIDTH: u32 = 480;
pub const BASE_HEIGHT: u32 = 96;
/// Per-section band heights for the expanded HUD.
pub const TASK_BAND_HEIGHT: u32 = 52;
pub const GITHUB_BAND_HEIGHT: u32 = 76;

#[derive(Clone, Copy, Debug, PartialEq)]
struct WorkArea {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

pub fn target_size(settings: &Settings) -> (u32, u32) {
    let height = BASE_HEIGHT
        + if settings.mini_bar_show_task {
            TASK_BAND_HEIGHT
        } else {
            0
        }
        + if settings.mini_bar_show_github {
            GITHUB_BAND_HEIGHT
        } else {
            0
        };
    (BASE_WIDTH, height)
}

pub fn apply(app: &tauri::AppHandle, settings: &Settings) -> Result<(), AppError> {
    let window = app
        .get_webview_window(WINDOW_LABEL)
        .ok_or(AppError::Window)?;
    if settings.mini_bar_enabled {
        let (width, height) = clamp_size(target_size(settings), &window)?;
        let current = window.outer_size().map_err(|_| AppError::Window)?;
        if current.width != width || current.height != height {
            window
                .set_size(PhysicalSize::new(width, height))
                .map_err(|_| AppError::Window)?;
        }
        if !window.is_visible().map_err(|_| AppError::Window)? {
            restore_position(&window, settings)?;
        }
        window.show().map_err(|_| AppError::Window)?;
        window
            .emit(REFRESH_EVENT, ())
            .map_err(|_| AppError::Window)?;
    } else {
        window.hide().map_err(|_| AppError::Window)?;
    }
    Ok(())
}

fn clamp_size((width, height): (u32, u32), window: &WebviewWindow) -> Result<(u32, u32), AppError> {
    let monitors = window.available_monitors().map_err(|_| AppError::Window)?;
    let max_height = monitors
        .iter()
        .map(|monitor| monitor.work_area().size.height)
        .max()
        .unwrap_or(height);
    let max_width = monitors
        .iter()
        .map(|monitor| monitor.work_area().size.width)
        .max()
        .unwrap_or(width);
    Ok((width.min(max_width), height.min(max_height)))
}

fn restore_position(window: &WebviewWindow, settings: &Settings) -> Result<(), AppError> {
    let size = window.outer_size().map_err(|_| AppError::Window)?;
    let monitors = window.available_monitors().map_err(|_| AppError::Window)?;
    let work_areas = monitors
        .iter()
        .map(|monitor| {
            let area = monitor.work_area();
            WorkArea {
                x: area.position.x,
                y: area.position.y,
                width: area.size.width,
                height: area.size.height,
            }
        })
        .collect::<Vec<_>>();
    let position = safe_position(
        settings.mini_bar_x.zip(settings.mini_bar_y),
        size.width,
        size.height,
        &work_areas,
    );
    window
        .set_position(PhysicalPosition::new(position.0, position.1))
        .map_err(|_| AppError::Window)
}

/// Keeps the entire bar in a monitor work area. A position from a removed
/// monitor is recentered instead of leaving an unreachable always-on-top window.
fn safe_position(
    saved: Option<(i32, i32)>,
    width: u32,
    height: u32,
    monitors: &[WorkArea],
) -> (i32, i32) {
    let Some(fallback) = monitors.first().copied() else {
        return saved.unwrap_or((0, 0));
    };
    if let Some((x, y)) = saved {
        if let Some(monitor) = monitors
            .iter()
            .copied()
            .find(|monitor| contains(*monitor, x, y))
        {
            return clamp_to(monitor, x, y, width, height);
        }
    }
    let center_x =
        i64::from(fallback.x) + (i64::from(fallback.width).saturating_sub(i64::from(width))) / 2;
    let center_y =
        i64::from(fallback.y) + (i64::from(fallback.height).saturating_sub(i64::from(height))) / 2;
    clamp_to(fallback, to_i32(center_x), to_i32(center_y), width, height)
}

fn contains(area: WorkArea, x: i32, y: i32) -> bool {
    let right = i64::from(area.x) + i64::from(area.width);
    let bottom = i64::from(area.y) + i64::from(area.height);
    i64::from(x) >= i64::from(area.x)
        && i64::from(x) < right
        && i64::from(y) >= i64::from(area.y)
        && i64::from(y) < bottom
}

fn clamp_to(area: WorkArea, x: i32, y: i32, width: u32, height: u32) -> (i32, i32) {
    let min_x = i64::from(area.x);
    let min_y = i64::from(area.y);
    let max_x = min_x + i64::from(area.width).saturating_sub(i64::from(width));
    let max_y = min_y + i64::from(area.height).saturating_sub(i64::from(height));
    (
        to_i32(i64::from(x).clamp(min_x, max_x.max(min_x))),
        to_i32(i64::from(y).clamp(min_y, max_y.max(min_y))),
    )
}

fn to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::{
        safe_position, target_size, WorkArea, BASE_HEIGHT, GITHUB_BAND_HEIGHT, TASK_BAND_HEIGHT,
    };

    fn settings(show_task: bool, show_github: bool) -> crate::settings::Settings {
        serde_json::from_str(&format!(
            r#"{{"closeToTray":true,"showMascot":true,"friendlyMessages":true,
            "miniBarShowTask":{show_task},"miniBarShowGitHub":{show_github}}}"#
        ))
        .expect("defaults")
    }

    #[test]
    fn size_is_quota_only_by_default_and_grows_per_enabled_section() {
        assert_eq!(target_size(&settings(false, false)), (480, BASE_HEIGHT));
        assert_eq!(
            target_size(&settings(true, false)),
            (480, BASE_HEIGHT + TASK_BAND_HEIGHT)
        );
        assert_eq!(
            target_size(&settings(false, true)),
            (480, BASE_HEIGHT + GITHUB_BAND_HEIGHT)
        );
        assert_eq!(
            target_size(&settings(true, true)),
            (480, BASE_HEIGHT + TASK_BAND_HEIGHT + GITHUB_BAND_HEIGHT)
        );
    }

    const PRIMARY: WorkArea = WorkArea {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };
    const LEFT: WorkArea = WorkArea {
        x: -1280,
        y: 0,
        width: 1280,
        height: 984,
    };

    #[test]
    fn retains_valid_multi_monitor_position_and_clamps_edges() {
        assert_eq!(
            safe_position(Some((-900, 200)), 640, 96, &[PRIMARY, LEFT]),
            (-900, 200)
        );
        assert_eq!(
            safe_position(Some((1800, 1000)), 640, 96, &[PRIMARY]),
            (1280, 944)
        );
    }

    #[test]
    fn recenters_position_from_removed_monitor() {
        assert_eq!(
            safe_position(Some((-900, 200)), 640, 96, &[PRIMARY]),
            (640, 472)
        );
        assert_eq!(safe_position(None, 640, 96, &[PRIMARY]), (640, 472));
    }
}
