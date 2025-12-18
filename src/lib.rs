use hound::{SampleFormat, WavReader, WavWriter};
use std::error::Error;
use std::path::Path;
use std::{fs, io};

// --- 判定結果の型 ---
#[derive(Debug, PartialEq, Copy, Clone)]
enum StereoType {
    DualMono,   // 実質モノラル
    TrueStereo, // ガチステレオ
}

fn is_dual_mono(path: &Path) -> hound::Result<StereoType> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();

    if spec.channels != 2 {
        return Ok(StereoType::DualMono);
    }

    let sample_rate = spec.sample_rate;
    let bits = spec.bits_per_sample;
    let format = spec.sample_format;

    // しきい値設定
    let silence_threshold = 10f32.powf(-60.0 / 20.0);
    let mono_diff_threshold = 10f32.powf(-60.0 / 20.0);
    let max_analyze_samples = 10 * sample_rate as usize;

    // 各型をf32に正規化するクロージャ
    // 24bitの場合は i32 として読み込み、2^23-1 で割る
    let to_f32 = move |sample: Result<i32, hound::Error>| -> f32 {
        let s = sample.unwrap_or(0);
        match (format, bits) {
            (SampleFormat::Int, 16) => s as f32 / i16::MAX as f32,
            (SampleFormat::Int, 24) => s as f32 / 8_388_607.0, // 2^23 - 1
            (SampleFormat::Int, 32) => s as f32 / i32::MAX as f32,
            _ => 0.0,
        }
    };

    // Houndのサンプルイテレータを正規化されたf32のイテレータに変換
    let mut samples: Box<dyn Iterator<Item = f32>> = match (format, bits) {
        (SampleFormat::Float, 32) => Box::new(reader.samples::<f32>().map(|s| s.unwrap_or(0.0))),
        (SampleFormat::Int, _) => Box::new(reader.samples::<i32>().map(to_f32)),
        _ => {
            return Err(hound::Error::IoError(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unsupported sample format for dual-mono check",
            )))
        }
    };

    let mut side_energy_sum = 0.0f64;
    let mut analyzed_count = 0usize;
    let mut is_started = false;
    let mut silence_samples = 0usize;

    // L/Rペアで回す
    while let (Some(l), Some(r)) = (samples.next(), samples.next()) {
        if !is_started {
            silence_samples += 1;
            if l.abs() > silence_threshold || r.abs() > silence_threshold {
                #[cfg(debug_assertions)]
                {
                    println!("Debug: l.abs() = {}", l.abs());
                    println!("Debug: r.abs() = {}", r.abs());
                    println!(
                        "Debug: silence seconds = {}",
                        silence_samples as f32 / sample_rate as f32
                    );
                }
                is_started = true;
            } else {
                continue;
            }
        }

        let side = (l - r) as f64;
        side_energy_sum += side * side;
        analyzed_count += 1;

        if analyzed_count >= max_analyze_samples {
            break;
        }
    }

    // サンプルが一つも解析されなかった場合は実質モノラルと見なす
    if analyzed_count == 0 {
        return Ok(StereoType::DualMono);
    }

    let side_rms = (side_energy_sum / analyzed_count as f64).sqrt() as f32;

    #[cfg(debug_assertions)]
    {
        println!("Debug: side_rms = {}", side_rms);
        println!("Debug: analyzed_count = {}", analyzed_count);
        println!("Debug: silence_threshold = {}", silence_threshold);
        println!("Debug: mono_diff_threshold = {}", mono_diff_threshold);
    }

    if side_rms < mono_diff_threshold {
        Ok(StereoType::DualMono)
    } else {
        Ok(StereoType::TrueStereo)
    }
}

/// 2-1. 1チャンネル目 (Lch) だけを抜き出す
/// 💡 S型のまま読み込み、S型のまま書き込むため、型不一致エラーは起きない！
fn extract_left_channel<S>(
    mut reader: WavReader<impl std::io::Read>,
    mut writer: WavWriter<impl std::io::Write + std::io::Seek>,
    channels: u16, // 2ch が渡されるハズ
) -> Result<(), hound::Error>
where
    S: hound::Sample + 'static,
{
    let mut samples = reader.samples::<S>();

    while let Some(l_res) = samples.next() {
        let l = l_res?;
        writer.write_sample(l)?; // Lch を書き込み

        // 2チャンネル目以降を読み飛ばす
        for _ in 1..channels {
            if samples.next().is_none() {
                break;
            }
        }
    }

    writer.finalize()?;
    Ok(())
}

// --- 3. メイン処理関数 ---

pub fn process_wav_file(input_path: &Path) -> Result<String, Box<dyn Error>> {
    // --- 3-1. 初期準備 ---
    let parent_dir = input_path.parent().ok_or("親フォルダが見つからないよ！")?;
    let file_name = input_path
        .file_name()
        .ok_or("ファイル名が取得できないよ！")?;
    let mono_dir = parent_dir.join("mono");
    let stereo_dir = parent_dir.join("stereo");
    let multichannel_dir = parent_dir.join("multichannel");
    let mono_output_path = mono_dir.join(file_name);
    let stereo_output_path = stereo_dir.join(file_name);
    let multichannel_output_path = multichannel_dir.join(file_name);

    // 最初に reader を開いて spec を取得 (DualMonoで再利用するかも)
    let reader = WavReader::open(input_path)?;
    let spec = reader.spec();

    // --- 3-2. チャンネル数で分岐 ---
    match spec.channels {
        // --- 1ch (モノラル) の場合 ---
        1 => {
            fs::create_dir_all(&mono_dir)?;
            fs::copy(input_path, mono_output_path)?;
            fs::remove_file(input_path)?;
            Ok(format!(
                "{} は 1ch だから 'mono' にコピーしたよ！",
                file_name.to_string_lossy()
            ))
        }

        // --- 2ch (ステレオ) の場合 ---
        2 => {
            let stereo_type = is_dual_mono(input_path)?;

            // 判定結果によって処理を分ける
            match stereo_type {
                // ガチステレオ (TrueStereo)
                StereoType::TrueStereo => {
                    fs::create_dir_all(&stereo_dir)?;
                    fs::copy(input_path, stereo_output_path)?;
                    fs::remove_file(input_path)?;
                    Ok(format!(
                        "{} はガチステレオだから 'stereo' にコピーしたよ！",
                        file_name.to_string_lossy()
                    ))
                }

                // 実質モノラル (DualMono)
                StereoType::DualMono => {
                    fs::create_dir_all(&mono_dir)?;

                    let mut mono_spec = spec;
                    mono_spec.channels = 1;

                    let writer = WavWriter::create(&mono_output_path, mono_spec)?;

                    // 💡 【修正点】抜き出し用の reader をここでファイル先頭から作り直す！
                    //    （前回のエラー対策）
                    let reader_for_extract = WavReader::open(input_path)?;

                    // 💡 【抜き出しブロック】 spec に合わせて抽出関数を呼び分ける！
                    match (spec.sample_format, spec.bits_per_sample) {
                        (SampleFormat::Int, 8) => {
                            extract_left_channel::<i8>(reader_for_extract, writer, spec.channels)?
                        }
                        (SampleFormat::Int, 16) => {
                            extract_left_channel::<i16>(reader_for_extract, writer, spec.channels)?
                        }
                        (SampleFormat::Int, 24) | (SampleFormat::Int, 32) => {
                            extract_left_channel::<i32>(reader_for_extract, writer, spec.channels)?
                        }
                        (SampleFormat::Float, 32) => {
                            extract_left_channel::<f32>(reader_for_extract, writer, spec.channels)?
                        }
                        // 判定ブロックで弾かれているので unreachable!
                        _ => unreachable!(),
                    }

                    Ok(format!(
                        "{} は実質モノラルだったから Lch を 'mono' に抜き出したよ！",
                        file_name.to_string_lossy()
                    ))
                }
            }
        }

        // --- 3ch 以上のファイル ---
        _ => {
            // copy multichannel files to "multichannel" folder
            fs::create_dir_all(&multichannel_dir)?;
            fs::copy(input_path, multichannel_output_path)?;
            fs::remove_file(input_path)?;
            Ok(format!(
                "{} は {}ch だから 'multichannel' にコピーしたよ！",
                file_name.to_string_lossy(),
                spec.channels
            ))
        }
    }
}
