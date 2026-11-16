use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct VideoFile {
    pub path: PathBuf,
    pub file_name: String,
    pub extension: String,
}

impl VideoFile {
    pub fn new(path: PathBuf) -> Option<Self> {
        let file_name = path.file_name()?.to_string_lossy().into_owned();

        let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();

        Some(Self {
            path,
            file_name,
            extension,
        })
    }
}
