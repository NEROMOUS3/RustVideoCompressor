use std::path::PathBuf;

use crate::video::VideoFile;

#[derive(Debug, Clone)]
pub struct OutputSettings {
    pub directory: PathBuf,
    pub file_name: String,
    pub extension: String,
}

impl OutputSettings {
    pub fn new(directory: PathBuf, file_name: String, extension: String) -> Self {
        Self {
            directory,
            file_name,
            extension,
        }
    }

    pub fn from_video(video: &VideoFile) -> Self {
        let directory = video
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();

        let source_name = video.path.file_stem().unwrap_or_default().to_string_lossy();

        let file_name = if source_name.ends_with("_compressed") {
            format!("{}_compressed_new", source_name)
        } else {
            format!("{}_compressed", source_name)
        };

        Self {
            directory,
            file_name,
            extension: video.extension.clone(),
        }
    }

    pub fn full_path(&self) -> PathBuf {
        self.directory
            .join(format!("{}.{}", self.file_name, self.extension))
    }
}
