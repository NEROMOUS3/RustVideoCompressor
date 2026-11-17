#[derive(Debug, Clone, PartialEq)]
pub enum ContainerFormat {
    Mp4,
    Mkv,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VideoCodec {
    H264,
    H265,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EncoderBackend {
    Software,
    NvidiaNvenc,
    AmdAmf,
    IntelQsv,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    Original,
    Preset(u32),
    Custom { width: u32, height: u32 },
}

#[derive(Debug, Clone)]
pub struct EncodingSettings {
    pub container: ContainerFormat,
    pub codec: VideoCodec,
    pub encoder: EncoderBackend,
    pub resolution: Resolution,
    pub multipass: bool,
    pub fast_start: bool,
    pub crf: u8,
    pub preset: String,
}

impl Default for EncodingSettings {
    fn default() -> Self {
        Self {
            container: ContainerFormat::Mp4,
            codec: VideoCodec::H264,
            encoder: EncoderBackend::Software,
            resolution: Resolution::Original,
            multipass: false,
            fast_start: true,
            crf: 23,
            preset: "medium".to_string(),
        }
    }
}

impl EncodingSettings {
    pub fn available_presets(&self) -> Vec<&'static str> {
        match self.encoder {
            EncoderBackend::Software => vec![
                "ultrafast",
                "superfast",
                "veryfast",
                "faster",
                "fast",
                "medium",
                "slow",
                "slower",
                "veryslow",
            ],

            EncoderBackend::NvidiaNvenc => vec!["P1", "P2", "P3", "P4", "P5", "P6", "P7"],

            EncoderBackend::AmdAmf => vec!["speed", "balanced", "quality"],

            EncoderBackend::IntelQsv => vec![
                "veryfast", "faster", "fast", "medium", "slow", "slower", "veryslow",
            ],
        }
    }

    pub fn default_preset(&self) -> &'static str {
        match self.encoder {
            EncoderBackend::Software => "medium",
            EncoderBackend::NvidiaNvenc => "P5",
            EncoderBackend::AmdAmf => "balanced",
            EncoderBackend::IntelQsv => "medium",
        }
    }
}

impl Resolution {
    pub fn available_presets() -> Vec<&'static str> {
        vec!["Оригинал", "2160p (4K)", "1440p", "1080p", "720p", "480p"]
    }

    pub fn default_index() -> i32 {
        0
    }

    pub fn from_index(index: i32) -> Self {
        match index {
            1 => Resolution::Preset(2160),
            2 => Resolution::Preset(1440),
            3 => Resolution::Preset(1080),
            4 => Resolution::Preset(720),
            5 => Resolution::Preset(480),
            _ => Resolution::Original,
        }
    }

    pub fn to_index(&self) -> i32 {
        match self {
            Resolution::Original => 0,
            Resolution::Preset(2160) => 1,
            Resolution::Preset(1440) => 2,
            Resolution::Preset(1080) => 3,
            Resolution::Preset(720) => 4,
            Resolution::Preset(480) => 5,
            Resolution::Preset(_) | Resolution::Custom { .. } => 0,
        }
    }

    pub fn calculate_dimensions(
        &self,
        source_width: u32,
        source_height: u32,
    ) -> Option<(u32, u32)> {
        if source_width == 0 || source_height == 0 {
            return None;
        }

        match self {
            Resolution::Original => Some((source_width, source_height)),

            Resolution::Preset(target_height) => {
                // Не увеличиваем видео, если исходная высота меньше выбранной.
                let height = (*target_height).min(source_height);

                if height == source_height {
                    return Some((source_width, source_height));
                }

                // Сохраняем пропорции исходного видео.
                let calculated_width = (source_width as u64 * height as u64) / source_height as u64;

                // Округляем ширину до ближайшего чётного числа.
                let width = ((calculated_width as u32 + 1) / 2) * 2;

                if width == 0 {
                    return None;
                }

                Some((width, height))
            }

            Resolution::Custom { .. } => {
                // Пользовательский размер реализуем позднее.
                None
            }
        }
    }
}

impl EncodingSettings {
    pub fn supports_multipass(&self) -> bool {
        match self.encoder {
            EncoderBackend::Software => true,
            EncoderBackend::NvidiaNvenc => true,
            EncoderBackend::AmdAmf => false,
            EncoderBackend::IntelQsv => false,
        }
    }

    pub fn supports_fast_start(&self) -> bool {
        matches!(self.container, ContainerFormat::Mp4)
    }

    pub fn normalize(&mut self) {
        if !self.supports_multipass() {
            self.multipass = false;
        }

        if !self.supports_fast_start() {
            self.fast_start = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Resolution;

    #[test]
    fn original_resolution_keeps_source_dimensions() {
        let resolution = Resolution::Original;

        assert_eq!(
            resolution.calculate_dimensions(1920, 1080),
            Some((1920, 1080))
        );
    }

    #[test]
    fn preset_resolution_preserves_aspect_ratio() {
        let resolution = Resolution::Preset(720);

        assert_eq!(
            resolution.calculate_dimensions(1920, 1080),
            Some((1280, 720))
        );
    }

    #[test]
    fn preset_resolution_rounds_width_to_even_number() {
        let resolution = Resolution::Preset(480);

        assert_eq!(
            resolution.calculate_dimensions(1920, 1080),
            Some((854, 480))
        );
    }

    #[test]
    fn preset_does_not_upscale_video() {
        let resolution = Resolution::Preset(1440);

        assert_eq!(
            resolution.calculate_dimensions(1920, 1080),
            Some((1920, 1080))
        );
    }

    #[test]
    fn zero_dimensions_return_none() {
        let resolution = Resolution::Preset(720);

        assert_eq!(resolution.calculate_dimensions(0, 1080), None);
        assert_eq!(resolution.calculate_dimensions(1920, 0), None);
    }

    #[test]
    fn custom_resolution_is_not_implemented_yet() {
        let resolution = Resolution::Custom {
            width: 1280,
            height: 720,
        };

        assert_eq!(resolution.calculate_dimensions(1920, 1080), None);
    }
}
