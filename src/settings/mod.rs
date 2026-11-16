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
