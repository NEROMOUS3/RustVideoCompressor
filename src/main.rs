slint::include_modules!();

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
use slint::{Timer, TimerMode};

use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

use crate::settings::{ContainerFormat, VideoCodec};
use app::AppState;

mod app;
mod ffmpeg;
mod output;
mod settings;
mod video;

// --------------------------------------------------
// Вспомогательные функции
// --------------------------------------------------

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

// --------------------------------------------------
// Синхронизация настроек с интерфейсом
// --------------------------------------------------

fn update_encoding_ui(window: &MainWindow, app_state: &Rc<RefCell<AppState>>) {
    let state = app_state.borrow();
    let settings = &state.encoding_settings;

    // Multipass и Fast Start
    window.set_multipass_available(settings.supports_multipass());
    window.set_fast_start_available(settings.supports_fast_start());

    window.set_multipass_enabled(settings.multipass);
    window.set_fast_start_enabled(settings.fast_start);

    // Контейнер
    window.set_container_index(match &settings.container {
        ContainerFormat::Mp4 => 0,
        ContainerFormat::Mkv => 1,
    });

    // Видеокодек
    window.set_codec_index(match &settings.codec {
        VideoCodec::H264 => 0,
        VideoCodec::H265 => 1,
    });
}

fn update_output_path_ui(window: &MainWindow, app_state: &Rc<RefCell<AppState>>) {
    let state = app_state.borrow();

    let output_path = state
        .output_settings
        .as_ref()
        .map(|settings| settings.full_path().display().to_string())
        .unwrap_or_default();

    window.set_output_full_path(output_path.into());
}

// --------------------------------------------------
// Выбор и обработка видео
// --------------------------------------------------

fn select_video(path: &Path, app_state: &Rc<RefCell<AppState>>, window: &MainWindow) {
    if !is_supported_video(path) {
        return;
    }

    let Some(video) = video::VideoFile::new(path.to_path_buf()) else {
        return;
    };

    // Завершаем редактирование предыдущего имени до смены видео.
    window.invoke_finish_output_name_editing();

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

    let output_settings = { app_state.borrow().output_settings.clone() };

    if let Some(settings) = output_settings {
        window.set_output_file_name(settings.file_name.into());

        window.set_output_directory(settings.directory.display().to_string().into());
    }

    update_output_path_ui(window, app_state);
}

// --------------------------------------------------
// Главная функция
// --------------------------------------------------

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;

    // Получение версий FFmpeg и FFprobe
    match ffmpeg::get_tool_version("ffmpeg") {
        Ok(version) => println!("FFmpeg: {}", version),
        Err(error) => println!("FFmpeg: {}", error),
    }

    match ffmpeg::get_tool_version("ffprobe") {
        Ok(version) => println!("FFprobe: {}", version),
        Err(error) => println!("FFprobe: {}", error),
    }

    // Создание состояния приложения
    let app_state = Rc::new(RefCell::new(AppState::default()));

    // Начальная синхронизация настроек
    update_encoding_ui(&main_window, &app_state);

    // --------------------------------------------------
    // Обработчики настроек кодирования
    // --------------------------------------------------

    // Multipass
    let weak_window_for_multipass = main_window.as_weak();
    let app_state_for_multipass = Rc::clone(&app_state);

    main_window.on_multipass_changed(move |enabled| {
        {
            let mut state = app_state_for_multipass.borrow_mut();
            let mut settings = state.encoding_settings.clone();

            settings.multipass = enabled;

            state.update_encoding_settings(settings);
        }

        if let Some(window) = weak_window_for_multipass.upgrade() {
            update_encoding_ui(&window, &app_state_for_multipass);
        }
    });

    // Fast Start
    let weak_window_for_fast_start = main_window.as_weak();
    let app_state_for_fast_start = Rc::clone(&app_state);

    main_window.on_fast_start_changed(move |enabled| {
        {
            let mut state = app_state_for_fast_start.borrow_mut();
            let mut settings = state.encoding_settings.clone();

            settings.fast_start = enabled;

            state.update_encoding_settings(settings);
        }

        if let Some(window) = weak_window_for_fast_start.upgrade() {
            update_encoding_ui(&window, &app_state_for_fast_start);
        }
    });

    // Контейнер
    let weak_window_for_container = main_window.as_weak();
    let app_state_for_container = Rc::clone(&app_state);

    main_window.on_container_changed(move |value| {
        let mut settings = { app_state_for_container.borrow().encoding_settings.clone() };

        settings.container = match value.as_str() {
            "MP4" => ContainerFormat::Mp4,
            "MKV" => ContainerFormat::Mkv,
            _ => return,
        };

        app_state_for_container
            .borrow_mut()
            .update_encoding_settings(settings);

        if let Some(window) = weak_window_for_container.upgrade() {
            update_encoding_ui(&window, &app_state_for_container);
            update_output_path_ui(&window, &app_state_for_container);
        }
    });

    // Видеокодек
    let weak_window_for_codec = main_window.as_weak();
    let app_state_for_codec = Rc::clone(&app_state);

    main_window.on_codec_changed(move |value| {
        let mut settings = { app_state_for_codec.borrow().encoding_settings.clone() };

        settings.codec = match value.as_str() {
            "H.264" => VideoCodec::H264,
            "H.265" => VideoCodec::H265,
            _ => return,
        };

        app_state_for_codec
            .borrow_mut()
            .update_encoding_settings(settings);

        if let Some(window) = weak_window_for_codec.upgrade() {
            update_encoding_ui(&window, &app_state_for_codec);
        }
    });

    // --------------------------------------------------
    // Drag & Drop
    // --------------------------------------------------

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

    // --------------------------------------------------
    // Обработчики выходного файла
    // --------------------------------------------------

    // Изменение имени выходного файла
    let weak_window_for_output_name = main_window.as_weak();
    let app_state_for_output_name = Rc::clone(&app_state);

    main_window.on_output_file_name_changed(move |new_name| {
        if let Some(settings) = app_state_for_output_name
            .borrow_mut()
            .output_settings
            .as_mut()
        {
            settings.file_name = new_name.to_string();
        }

        if let Some(window) = weak_window_for_output_name.upgrade() {
            update_output_path_ui(&window, &app_state_for_output_name);
        }
    });

    // Завершение редактирования имени
    let weak_window_for_finish = main_window.as_weak();
    let app_state_for_finish = Rc::clone(&app_state);

    main_window.on_output_file_name_editing_finished(move || {
        let updated_name = app_state_for_finish
            .borrow_mut()
            .finish_output_name_editing();

        if let Some(window) = weak_window_for_finish.upgrade() {
            if let Some(name) = updated_name {
                window.set_output_file_name(name.into());
            }

            update_output_path_ui(&window, &app_state_for_finish);
        }
    });

    // --------------------------------------------------
    // Выбор папки сохранения
    // --------------------------------------------------

    let weak_window_for_directory = main_window.as_weak();
    let app_state_for_directory = Rc::clone(&app_state);

    main_window.on_choose_output_directory(move || {
        let current_directory = { app_state_for_directory.borrow().output_directory.clone() };

        let mut dialog = rfd::FileDialog::new();

        if let Some(directory) = current_directory {
            dialog = dialog.set_directory(directory);
        }

        if let Some(directory) = dialog.pick_folder() {
            app_state_for_directory
                .borrow_mut()
                .set_output_directory(directory.clone());

            if let Some(window) = weak_window_for_directory.upgrade() {
                window.set_output_directory(directory.display().to_string().into());

                update_output_path_ui(&window, &app_state_for_directory);
            }
        }
    });

    // --------------------------------------------------
    // Выбор видео через диалог
    // --------------------------------------------------

    let weak_window_for_file = main_window.as_weak();
    let app_state_for_dialog = Rc::clone(&app_state);

    main_window.on_choose_file(move || {
        let file = rfd::FileDialog::new()
            .add_filter("Видео", &["mp4", "mkv", "mov", "avi", "webm", "m4v"])
            .pick_file();

        if let Some(path) = file {
            if let Some(window) = weak_window_for_file.upgrade() {
                select_video(&path, &app_state_for_dialog, &window);
            }
        }
    });

    // Запуск приложения
    main_window.run()
}
