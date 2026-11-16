use std::path::PathBuf;

use crate::output::OutputSettings;
use crate::settings::{ContainerFormat, EncodingSettings};
use crate::video::{VideoFile, VideoInfo};

#[derive(Debug, Default)]
pub struct AppState {
    pub selected_video: Option<VideoFile>,
    pub output_settings: Option<OutputSettings>,
    pub output_directory: Option<PathBuf>,
    pub video_info: Option<VideoInfo>,
    pub encoding_settings: EncodingSettings,
}

impl AppState {
    pub fn select_video(&mut self, video: VideoFile) {
        self.video_info = None;

        let mut output_settings = OutputSettings::from_video(&video);

        if let Some(directory) = &self.output_directory {
            output_settings.directory = directory.clone();
        }

        // Применяем расширение выбранного контейнера
        output_settings.set_extension(self.container_extension());

        self.output_directory = Some(output_settings.directory.clone());
        self.output_settings = Some(output_settings);

        self.selected_video = Some(video);
    }

    pub fn set_output_directory(&mut self, directory: PathBuf) {
        self.output_directory = Some(directory.clone());

        if let Some(settings) = self.output_settings.as_mut() {
            settings.directory = directory;
        }
    }

    pub fn finish_output_name_editing(&mut self) -> Option<String> {
        let output_settings = self.output_settings.as_mut()?;

        if output_settings.file_name.trim().is_empty() {
            let video = self.selected_video.as_ref()?;
            let default_settings = OutputSettings::from_video(video);

            output_settings.file_name = default_settings.file_name;
        }

        Some(output_settings.file_name.clone())
    }

    pub fn set_video_info(&mut self, info: VideoInfo) {
        self.video_info = Some(info);
    }

    pub fn update_encoding_settings(&mut self, settings: EncodingSettings) {
        let mut settings = settings;
        settings.normalize();

        self.encoding_settings = settings;

        // Обновляем расширение выходного файла
        let extension = self.container_extension();

        if let Some(output_settings) = self.output_settings.as_mut() {
            output_settings.set_extension(extension);
        }
    }

    fn container_extension(&self) -> String {
        match self.encoding_settings.container {
            ContainerFormat::Mp4 => "mp4",
            ContainerFormat::Mkv => "mkv",
        }
        .to_string()
    }
}
