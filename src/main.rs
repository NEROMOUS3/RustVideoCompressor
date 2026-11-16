slint::include_modules!();

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
use slint::{Timer, TimerMode};

use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

use app::AppState;

mod app;
mod ffmpeg;
mod output;
mod video;

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
                && cursor_y <= 304.0
        })
        .unwrap_or(false)
}

fn is_supported_video(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("mp4" | "mkv" | "mov" | "avi" | "webm" | "m4v")
    )
}

fn format_duration(seconds: f64) -> String {
    let total_seconds = seconds.round() as u64;

    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn format_file_size(bytes: u64) -> String {
    let megabytes = bytes as f64 / (1024.0 * 1024.0);

    format!("{:.2} МБ", megabytes)
}

fn select_video(path: &Path, app_state: &Rc<RefCell<AppState>>, window: &MainWindow) {
    if !is_supported_video(path) {
        return;
    }

    let Some(video) = video::VideoFile::new(path.to_path_buf()) else {
        return;
    };

    window.set_selected_file(video.file_name.clone().into());

    app_state.borrow_mut().select_video(video);

    let selected_video = { app_state.borrow().selected_video.clone() };

    if let Some(video) = selected_video {
        match ffmpeg::get_video_info(&video.path) {
            Ok(info) => {
                // Передаём информацию в интерфейс
                window.set_video_resolution(format!("{} × {}", info.width, info.height).into());

                window.set_video_duration(format_duration(info.duration).into());

                window.set_video_fps(format!("{:.2}", info.fps).into());

                window.set_video_codec(info.codec.clone().into());

                window.set_video_size(format_file_size(info.size).into());

                // Сохраняем информацию в состоянии приложения
                app_state.borrow_mut().set_video_info(info);
            }
            Err(error) => {
                println!("Ошибка получения информации: {}", error);
            }
        }
    }

    let output_settings = app_state.borrow().output_settings.clone();

    if let Some(settings) = output_settings {
        window.invoke_finish_output_name_editing();

        window.set_output_file_name(settings.file_name.into());
        window.set_output_directory(settings.directory.display().to_string().into());
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;

    match ffmpeg::get_tool_version("ffmpeg") {
        Ok(version) => println!("FFmpeg: {}", version),
        Err(error) => println!("FFmpeg: {}", error),
    }

    match ffmpeg::get_tool_version("ffprobe") {
        Ok(version) => println!("FFprobe: {}", version),
        Err(error) => println!("FFprobe: {}", error),
    }

    let app_state = Rc::new(RefCell::new(AppState::default()));

    let weak_window = main_window.as_weak();
    let weak_window_for_timer = weak_window.clone();

    let app_state_for_drop = Rc::clone(&app_state);

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

                    if is_inside {
                        select_video(path, &app_state_for_drop, &window);
                    }
                }

                _ => {}
            }
        }

        EventResult::Propagate
    });

    // Обработка кнопки выбора файла
    let weak_window = main_window.as_weak();
    let app_state_for_dialog = Rc::clone(&app_state);

    let app_state_for_output_name = Rc::clone(&app_state);

    main_window.on_output_file_name_changed(move |new_name| {
        if let Some(settings) = app_state_for_output_name
            .borrow_mut()
            .output_settings
            .as_mut()
        {
            settings.file_name = new_name.to_string();
        }
    });

    let weak_window_for_output = main_window.as_weak();
    let app_state_for_finish = Rc::clone(&app_state);

    main_window.on_output_file_name_editing_finished(move || {
        let updated_name = app_state_for_finish
            .borrow_mut()
            .finish_output_name_editing();

        if let (Some(name), Some(window)) = (updated_name, weak_window_for_output.upgrade()) {
            window.set_output_file_name(name.into());
        }
    });

    // Обработка кнопки выбора папки сохранения
    let weak_window_for_directory = main_window.as_weak();
    let app_state_for_directory = Rc::clone(&app_state);

    main_window.on_choose_output_directory(move || {
        // Получаем текущую папку, чтобы открыть диалог в ней
        let current_directory = { app_state_for_directory.borrow().output_directory.clone() };

        let mut dialog = rfd::FileDialog::new();

        if let Some(directory) = current_directory {
            dialog = dialog.set_directory(directory);
        }

        if let Some(directory) = dialog.pick_folder() {
            // Сохраняем выбранную папку
            app_state_for_directory
                .borrow_mut()
                .set_output_directory(directory.clone());

            // Обновляем путь в интерфейсе
            if let Some(window) = weak_window_for_directory.upgrade() {
                window.set_output_directory(directory.display().to_string().into());
            }
        }
    });

    main_window.on_choose_file(move || {
        let file = rfd::FileDialog::new()
            .add_filter("Видео", &["mp4", "mkv", "mov", "avi", "webm", "m4v"])
            .pick_file();

        if let Some(path) = file {
            if let Some(window) = weak_window.upgrade() {
                select_video(&path, &app_state_for_dialog, &window);
            }
        }
    });

    main_window.run()
}
