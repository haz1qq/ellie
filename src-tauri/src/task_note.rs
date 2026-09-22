use tauri::{Emitter, Manager, PhysicalPosition, WebviewWindow};

use crate::{error::AppError, tasks::TaskNoteSnapshot};

pub const WINDOW_LABEL: &str = "task-note";
pub const UPDATE_EVENT: &str = "task-note-updated";
pub const TASKS_UPDATED_EVENT: &str = "tasks-updated";

#[derive(Clone, Copy, Debug, PartialEq)]
struct WorkArea {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// Keeps the dedicated sticky note synchronized with the one locally pinned
/// task. Task data is emitted only to this least-privilege window.
pub fn apply(app: &tauri::AppHandle, snapshot: &TaskNoteSnapshot) -> Result<(), AppError> {
    let window = app
        .get_webview_window(WINDOW_LABEL)
        .ok_or(AppError::Window)?;
    window
        .emit(UPDATE_EVENT, &snapshot.task)
        .map_err(|_| AppError::Window)?;
    if snapshot.task.is_some() {
        if !window.is_visible().map_err(|_| AppError::Window)? {
            restore_position(&window, snapshot.position)?;
        }
        window.show().map_err(|_| AppError::Window)?;
    } else {
        window.hide().map_err(|_| AppError::Window)?;
    }
    Ok(())
}

pub fn notify_main(app: &tauri::AppHandle) -> Result<(), AppError> {
    app.get_webview_window("main")
        .ok_or(AppError::Window)?
        .emit(TASKS_UPDATED_EVENT, ())
        .map_err(|_| AppError::Window)
}

/// Keeps the expanded HUD's current-task section fresh after task changes.
pub fn notify_mini(app: &tauri::AppHandle) -> Result<(), AppError> {
    app.get_webview_window(super::mini_bar::WINDOW_LABEL)
        .ok_or(AppError::Window)?
        .emit(TASKS_UPDATED_EVENT, ())
        .map_err(|_| AppError::Window)
}

fn restore_position(window: &WebviewWindow, saved: Option<(i32, i32)>) -> Result<(), AppError> {
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
    let (x, y) = safe_position(saved, size.width, size.height, &work_areas);
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|_| AppError::Window)
}

/// Re-centers a note saved on a removed display and clamps partially off-screen
/// positions, so an always-on-top note can never become unreachable.
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
    use super::{safe_position, WorkArea};

    const PRIMARY: WorkArea = WorkArea {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };

    #[test]
    fn retains_valid_positions_and_clamps_edges() {
        assert_eq!(
            safe_position(Some((300, 200)), 360, 300, &[PRIMARY]),
            (300, 200)
        );
        assert_eq!(
            safe_position(Some((1900, 1000)), 360, 300, &[PRIMARY]),
            (1560, 740)
        );
    }

    #[test]
    fn recenters_a_position_from_a_removed_monitor() {
        assert_eq!(
            safe_position(Some((-900, 200)), 360, 300, &[PRIMARY]),
            (780, 370)
        );
    }
}
