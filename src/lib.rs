use hound::{SampleFormat, WavReader, WavWriter};
use std::error::Error;
use std::fs;
use std::path::Path;

// --- 判定結果の型 ---
#[derive(Debug, PartialEq, Copy, Clone)]
enum StereoType {
    DualMono,   // 実質モノラル
    TrueStereo, // ガチステレオ
}

// --- 1. 判定関数 (Int/Float 呼び分け用) ---

/// 1-1. 整数形式 (Int) の判定関数 (許容範囲付き)
/// 💡 (l - r).abs() > TOLERANCE で比較
fn check_stereo_type_int<S>(
    mut reader: WavReader<impl std::io::Read>,
) -> Result<StereoType, hound::Error>
where
    S: hound::Sample + Copy + 'static,
{
    // 許容するLSBの数。2 LSBs までをノイズと見なす！
    const INT_TOLERANCE: i16 = 2;

    let mut samples = reader.samples::<S>();
    let mut cnt = 0;
    while let (Some(l_res), Some(r_res)) = (samples.next(), samples.next()) {
        // 💡 i64 にキャストして計算 (符号付き整数ならすべて安全に計算できる)
        let l = l_res?.as_i16();
        let r = r_res?.as_i16();

        let diff = (l - r).abs();

        if diff > INT_TOLERANCE {
            println!(
                "Debug: l = {}, r = {}, diff = {}, cnt = {}",
                l, r, diff, cnt
            );
            // 許容範囲を超えたらステレオ確定！
            return Ok(StereoType::TrueStereo);
        }

        if cnt >= 1_000_000 {
            // 100万サンプル調べたら打ち切り
            break;
        }
        cnt += 1;
    }
    Ok(StereoType::DualMono)
}
/// 1-2. 浮動小数点形式 (Float, f32) の判定関数
/// 💡 (l - r).abs() > MONO_EPSILON の許容範囲比較
fn check_stereo_type_float(
    mut reader: WavReader<impl std::io::Read>,
) -> Result<StereoType, hound::Error> {
    // 許容範囲: 16bitの約3ステップ分くらい
    const MONO_EPSILON: f32 = 0.0001;
    let mut samples = reader.samples::<f32>();

    while let (Some(l_res), Some(r_res)) = (samples.next(), samples.next()) {
        if (l_res? - r_res?).abs() > MONO_EPSILON {
            // 差が許容範囲を超えたらガチステレオ確定！
            return Ok(StereoType::TrueStereo);
        }
    }
    Ok(StereoType::DualMono)
}

// --- 2. 抜き出し関数 (ジェネリック版) ---

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
            Ok(format!(
                "{} は 1ch だから 'mono' にコピーしたよ！",
                file_name.to_string_lossy()
            ))
        }

        // --- 2ch (ステレオ) の場合 ---
        2 => {
            // 💡 【判定ブロック】 spec に合わせて判定関数を呼び分ける！
            let stereo_type = match (spec.sample_format, spec.bits_per_sample) {
                // Int 形式なら Int 用の厳密判定を呼ぶ
                (SampleFormat::Int, 8) => {
                    check_stereo_type_int::<i8>(WavReader::open(input_path)?)?
                }
                (SampleFormat::Int, 16) => {
                    check_stereo_type_int::<i16>(WavReader::open(input_path)?)?
                }
                // 24bit/32bit Int は i32 で読む
                (SampleFormat::Int, 24) | (SampleFormat::Int, 32) => {
                    check_stereo_type_int::<i32>(WavReader::open(input_path)?)?
                }

                // Float 形式なら Float 用のイプシロン判定を呼ぶ
                (SampleFormat::Float, 32) => check_stereo_type_float(WavReader::open(input_path)?)?,

                _ => {
                    return Err(Box::from(format!(
                        "2ch だけど、この形式 ({:?} / {} bits) は対応してないかも...ごめん！",
                        spec.sample_format, spec.bits_per_sample
                    )));
                }
            };

            // 判定結果によって処理を分ける
            match stereo_type {
                // ガチステレオ (TrueStereo)
                StereoType::TrueStereo => {
                    fs::create_dir_all(&stereo_dir)?;
                    fs::copy(input_path, stereo_output_path)?;
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
            Ok(format!(
                "{} は {}ch だから 'multichannel' にコピーしたよ！",
                file_name.to_string_lossy(),
                spec.channels
            ))
        }
    }
}
