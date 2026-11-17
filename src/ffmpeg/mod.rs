use std::process::Command;

use std::path::Path;

use serde::Deserialize;

use crate::video::VideoInfo;

use std::ffi::OsString;

use crate::settings::{ContainerFormat, EncoderBackend, EncodingSettings, Resolution, VideoCodec};

pub fn get_tool_version(executable: &str) -> Result<String, String> {
    let output = Command::new(executable)
        .arg("-version")
        .output()
        .map_err(|error| format!("Не удалось запустить {}: {}", executable, error))?;

    if !output.status.success() {
        return Err(format!(
            "{} завершился с ошибкой: {}",
            executable,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let version_output = String::from_utf8_lossy(&output.stdout);

    let version = version_output
        .lines()
        .next()
        .unwrap_or("Версия неизвестна")
        .to_string();

    Ok(version)
}

#[derive(Debug, Deserialize)]
struct ProbeOutput {
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}

#[derive(Debug, Deserialize)]
struct ProbeStream {
    width: u32,
    height: u32,
    codec_name: String,
    avg_frame_rate: String,
    r_frame_rate: String,
}

#[derive(Debug, Deserialize)]
struct ProbeFormat {
    duration: Option<String>,
    size: Option<String>,
}

fn parse_frame_rate(value: &str) -> Option<f64> {
    let (numerator, denominator) = value.split_once('/')?;

    let numerator: f64 = numerator.parse().ok()?;
    let denominator: f64 = denominator.parse().ok()?;

    if denominator == 0.0 {
        return None;
    }

    Some(numerator / denominator)
}

pub fn get_video_info(path: &Path) -> Result<VideoInfo, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,codec_name,avg_frame_rate,r_frame_rate:format=duration,size",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|error| format!("Не удалось запустить FFprobe: {}", error))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let json = String::from_utf8_lossy(&output.stdout);

    let probe: ProbeOutput = serde_json::from_str(&json)
        .map_err(|error| format!("Ошибка чтения данных FFprobe: {}", error))?;

    let stream = probe
        .streams
        .first()
        .ok_or("В файле не найден видеопоток")?;

    let format = probe
        .format
        .ok_or("FFprobe не вернул информацию о формате")?;

    let duration = format
        .duration
        .ok_or("Не удалось определить длительность видео")?
        .parse::<f64>()
        .map_err(|error| format!("Ошибка длительности видео: {}", error))?;

    let size = format
        .size
        .ok_or("Не удалось определить размер файла")?
        .parse::<u64>()
        .map_err(|error| format!("Ошибка размера файла: {}", error))?;

    let fps = parse_frame_rate(&stream.avg_frame_rate)
        .filter(|fps| *fps > 0.0)
        .or_else(|| parse_frame_rate(&stream.r_frame_rate))
        .ok_or("Не удалось определить FPS")?;

    Ok(VideoInfo {
        width: stream.width,
        height: stream.height,
        duration,
        codec: stream.codec_name.clone(),
        fps,
        size,
    })
}

pub fn build_ffmpeg_args(
    input: &Path,
    output: &Path,
    video_info: &VideoInfo,
    settings: &EncodingSettings,
) -> Result<Vec<OsString>, String> {
    // Пока не реализовали двухпроходное кодирование.
    if settings.multipass {
        return Err("Режим Multipass пока не реализован".to_string());
    }

    // Определяем видеокодек FFmpeg.
    let video_encoder = match (&settings.codec, &settings.encoder) {
        (VideoCodec::H264, EncoderBackend::Software) => "libx264",
        (VideoCodec::H265, EncoderBackend::Software) => "libx265",

        (VideoCodec::H264, EncoderBackend::NvidiaNvenc) => "h264_nvenc",
        (VideoCodec::H265, EncoderBackend::NvidiaNvenc) => "hevc_nvenc",

        (VideoCodec::H264, EncoderBackend::AmdAmf) => "h264_amf",
        (VideoCodec::H265, EncoderBackend::AmdAmf) => "hevc_amf",

        (VideoCodec::H264, EncoderBackend::IntelQsv) => "h264_qsv",
        (VideoCodec::H265, EncoderBackend::IntelQsv) => "hevc_qsv",
    };

    let mut args: Vec<OsString> = Vec::new();

    // Перезаписывать существующий выходной файл.
    args.push("-y".into());

    // Входной файл.
    args.push("-i".into());
    args.push(input.as_os_str().to_owned());

    // Выбираем первый видеопоток и все аудиопотоки, если они есть.
    args.push("-map".into());
    args.push("0:v:0".into());

    args.push("-map".into());
    args.push("0:a?".into());

    // Видеокодек.
    args.push("-c:v".into());
    args.push(video_encoder.into());

    // Пресет кодировщика.
    args.push("-preset".into());
    args.push(settings.preset.as_str().into());

    // Параметры качества в зависимости от кодировщика.
    match settings.encoder {
        EncoderBackend::Software => {
            args.push("-crf".into());
            args.push(settings.crf.to_string().into());
        }

        EncoderBackend::NvidiaNvenc => {
            args.push("-rc".into());
            args.push("vbr".into());

            args.push("-cq".into());
            args.push(settings.crf.to_string().into());
        }

        EncoderBackend::AmdAmf => {
            args.push("-rc".into());
            args.push("cqp".into());

            args.push("-qp_i".into());
            args.push(settings.crf.to_string().into());

            args.push("-qp_p".into());
            args.push(settings.crf.to_string().into());
        }

        EncoderBackend::IntelQsv => {
            args.push("-global_quality".into());
            args.push(settings.crf.to_string().into());
        }
    }

    // Рассчитываем выходное разрешение.
    let dimensions = settings
        .resolution
        .calculate_dimensions(video_info.width, video_info.height)
        .ok_or_else(|| "Не удалось рассчитать выходное разрешение".to_string())?;

    if dimensions != (video_info.width, video_info.height) {
        let (width, height) = dimensions;

        args.push("-vf".into());
        args.push(format!("scale={}:{}:flags=lanczos", width, height).into());
    }

    // Формат пикселей для совместимости.
    args.push("-pix_fmt".into());
    args.push("yuv420p".into());

    // Аудио кодируем в AAC.
    args.push("-c:a".into());
    args.push("aac".into());

    args.push("-b:a".into());
    args.push("192k".into());

    // Сохраняем метаданные исходного файла.
    args.push("-map_metadata".into());
    args.push("0".into());

    // Fast Start поддерживается для MP4.
    if settings.fast_start
        && settings.supports_fast_start()
        && matches!(settings.container, ContainerFormat::Mp4)
    {
        args.push("-movflags".into());
        args.push("+faststart".into());
    }

    // Выходной файл.
    args.push(output.as_os_str().to_owned());

    Ok(args)
}

pub fn check_tools() -> Result<(), String> {
    let mut errors = Vec::new();

    if let Err(error) = get_tool_version("ffmpeg") {
        errors.push(format!("FFmpeg: {}", error));
    }

    if let Err(error) = get_tool_version("ffprobe") {
        errors.push(format!("FFprobe: {}", error));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::build_ffmpeg_args;
    use crate::settings::{
        ContainerFormat, EncoderBackend, EncodingSettings, Resolution, VideoCodec,
    };
    use crate::video::VideoInfo;
    use std::path::Path;

    fn create_video_info() -> VideoInfo {
        VideoInfo {
            width: 1920,
            height: 1080,
            duration: 120.0,
            codec: "h264".to_string(),
            fps: 30.0,
            size: 10_000_000,
        }
    }

    fn args_to_strings(args: Vec<std::ffi::OsString>) -> Vec<String> {
        args.into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn software_h264_uses_libx264_and_crf() {
        let settings = EncodingSettings::default();

        let args = build_ffmpeg_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            &create_video_info(),
            &settings,
        )
        .unwrap();

        let args = args_to_strings(args);

        assert!(args.windows(2).any(|pair| pair == ["-c:v", "libx264"]));
        assert!(args.windows(2).any(|pair| pair == ["-crf", "23"]));
        assert!(args.windows(2).any(|pair| pair == ["-preset", "medium"]));
    }

    #[test]
    fn software_h265_uses_libx265() {
        let mut settings = EncodingSettings::default();
        settings.codec = VideoCodec::H265;

        let args = build_ffmpeg_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            &create_video_info(),
            &settings,
        )
        .unwrap();

        let args = args_to_strings(args);

        assert!(args.windows(2).any(|pair| pair == ["-c:v", "libx265"]));
    }

    #[test]
    fn nvidia_encoder_uses_nvenc() {
        let mut settings = EncodingSettings::default();
        settings.encoder = EncoderBackend::NvidiaNvenc;
        settings.preset = "P5".to_string();

        let args = build_ffmpeg_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            &create_video_info(),
            &settings,
        )
        .unwrap();

        let args = args_to_strings(args);

        assert!(args.windows(2).any(|pair| pair == ["-c:v", "h264_nvenc"]));
        assert!(args.windows(2).any(|pair| pair == ["-cq", "23"]));
        assert!(args.windows(2).any(|pair| pair == ["-preset", "P5"]));
    }

    #[test]
    fn amd_encoder_uses_amf() {
        let mut settings = EncodingSettings::default();
        settings.encoder = EncoderBackend::AmdAmf;
        settings.preset = "balanced".to_string();

        let args = build_ffmpeg_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            &create_video_info(),
            &settings,
        )
        .unwrap();

        let args = args_to_strings(args);

        assert!(args.windows(2).any(|pair| pair == ["-c:v", "h264_amf"]));
        assert!(args.windows(2).any(|pair| pair == ["-rc", "cqp"]));
        assert!(args.windows(2).any(|pair| pair == ["-qp_i", "23"]));
    }

    #[test]
    fn intel_encoder_uses_qsv() {
        let mut settings = EncodingSettings::default();
        settings.encoder = EncoderBackend::IntelQsv;

        let args = build_ffmpeg_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            &create_video_info(),
            &settings,
        )
        .unwrap();

        let args = args_to_strings(args);

        assert!(args.windows(2).any(|pair| pair == ["-c:v", "h264_qsv"]));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["-global_quality", "23"])
        );
    }

    #[test]
    fn resolution_adds_scale_filter() {
        let mut settings = EncodingSettings::default();
        settings.resolution = Resolution::Preset(720);

        let args = build_ffmpeg_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            &create_video_info(),
            &settings,
        )
        .unwrap();

        let args = args_to_strings(args);

        assert!(
            args.windows(2)
                .any(|pair| pair == ["-vf", "scale=1280:720:flags=lanczos"])
        );
    }

    #[test]
    fn original_resolution_does_not_add_scale_filter() {
        let settings = EncodingSettings::default();

        let args = build_ffmpeg_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            &create_video_info(),
            &settings,
        )
        .unwrap();

        let args = args_to_strings(args);

        assert!(!args.contains(&"-vf".to_string()));
    }

    #[test]
    fn fast_start_is_added_for_mp4() {
        let settings = EncodingSettings::default();

        let args = build_ffmpeg_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            &create_video_info(),
            &settings,
        )
        .unwrap();

        let args = args_to_strings(args);

        assert!(
            args.windows(2)
                .any(|pair| pair == ["-movflags", "+faststart"])
        );
    }

    #[test]
    fn fast_start_is_not_added_for_mkv() {
        let mut settings = EncodingSettings::default();
        settings.container = ContainerFormat::Mkv;
        settings.normalize();

        let args = build_ffmpeg_args(
            Path::new("input.mp4"),
            Path::new("output.mkv"),
            &create_video_info(),
            &settings,
        )
        .unwrap();

        let args = args_to_strings(args);

        assert!(!args.contains(&"-movflags".to_string()));
    }

    #[test]
    fn multipass_returns_error() {
        let mut settings = EncodingSettings::default();
        settings.multipass = true;

        let result = build_ffmpeg_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            &create_video_info(),
            &settings,
        );

        assert!(result.is_err());
    }
}
