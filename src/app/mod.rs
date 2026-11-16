use crate::output::OutputSettings;
use crate::video::VideoFile;

#[derive(Debug, Default)]
pub struct AppState {
    pub selected_video: Option<VideoFile>,
    pub output_settings: Option<OutputSettings>,
}

impl AppState {
    pub fn select_video(&mut self, video: VideoFile) {
        self.output_settings = Some(OutputSettings::from_video(&video));
        self.selected_video = Some(video);
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
}
