use std::process::Command;

use std::path::Path;

use serde::Deserialize;

use crate::video::VideoInfo;

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
