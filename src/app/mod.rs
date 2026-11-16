use std::path::PathBuf;

use crate::output::OutputSettings;
use crate::video::{VideoFile, VideoInfo};

#[derive(Debug, Default)]
pub struct AppState {
    pub selected_video: Option<VideoFile>,
    pub output_settings: Option<OutputSettings>,
    pub output_directory: Option<PathBuf>,
    pub video_info: Option<VideoInfo>,
}

impl AppState {
    pub fn select_video(&mut self, video: VideoFile) {
        self.video_info = None;

        let mut output_settings = OutputSettings::from_video(&video);

        if let Some(directory) = &self.output_directory {
            output_settings.directory = directory.clone();
        }

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
}
