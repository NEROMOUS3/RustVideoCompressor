slint::include_modules!();

use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
use std::rc::Rc;
use std::time::Duration;

use slint::{Timer, TimerMode};

use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

fn is_cursor_inside_drop_zone(window: &MainWindow) -> bool {
    let mut cursor_position = POINT { x: 0, y: 0 };

    let success = unsafe { GetCursorPos(&mut cursor_position) };

    if success == 0 {
        return false;
    }

    window
        .window()
        .with_winit_window(|winit_window| {
            let Ok(window_position) = winit_window.inner_position() else {
                return false;
            };

            let scale_factor = winit_window.scale_factor();

            let cursor_x = (cursor_position.x - window_position.x) as f64 / scale_factor;

            let cursor_y = (cursor_position.y - window_position.y) as f64 / scale_factor;

            let window_width = winit_window.inner_size().width as f64 / scale_factor;

            cursor_x >= 24.0
                && cursor_x <= window_width - 24.0
                && cursor_y >= 24.0
                && cursor_y <= 154.0
        })
        .unwrap_or(false)
}

fn is_supported_video(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("mp4" | "mkv" | "mov" | "avi" | "webm" | "m4v")
    )
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;
    let weak_window = main_window.as_weak();
    let weak_window_for_timer = weak_window.clone();

    let drag_timer = Rc::new(Timer::default());

    main_window.window().on_winit_window_event(move |_, event| {
        if let Some(window) = weak_window.upgrade() {
            match event {
                winit::event::WindowEvent::HoveredFile(_) => {
                    window.set_is_dragging_file(true);

                    let weak_window = weak_window_for_timer.clone();

                    drag_timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
                        if let Some(window) = weak_window.upgrade() {
                            let is_inside = is_cursor_inside_drop_zone(&window);

                            window.set_drop_zone_hovered(is_inside);
                        }
                    });
                }

                winit::event::WindowEvent::HoveredFileCancelled => {
                    drag_timer.stop();

                    window.set_is_dragging_file(false);
                    window.set_drop_zone_hovered(false);
                }

                winit::event::WindowEvent::DroppedFile(path) => {
                    let is_inside = is_cursor_inside_drop_zone(&window);

                    drag_timer.stop();

                    window.set_is_dragging_file(false);
                    window.set_drop_zone_hovered(false);

                    if is_inside && is_supported_video(path) {
                        window.set_selected_file(path.display().to_string().into());
                    }
                }

                _ => {}
            }
        }

        EventResult::Propagate
    });
    // Обработка кнопки выбора файла
    let weak_window = main_window.as_weak();

    main_window.on_choose_file(move || {
        let file = rfd::FileDialog::new()
            .add_filter("Видео", &["mp4", "mkv", "mov", "avi", "webm", "m4v"])
            .pick_file();

        if let Some(path) = file {
            if let Some(window) = weak_window.upgrade() {
                window.set_selected_file(path.display().to_string().into());
            }
        }
    });

    main_window.run()
}
