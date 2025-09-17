use std::f32::consts::PI;

/// テスト用のWAVファイルを生成（800Hz、0.5秒の正弦波）
pub fn generate_test_audio(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let sample_rate = 44100;
    let duration = 0.5; // 0.5秒
    let frequency = 800.0; // 800Hz
    let amplitude = 0.5; // 音量

    let num_samples = (sample_rate as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    // 正弦波を生成
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = amplitude * (2.0 * PI * frequency * t).sin();
        samples.push(sample);
    }

    // WAV形式で保存
    write_wav_file(path, &samples, sample_rate)?;
    Ok(())
}

fn write_wav_file(path: &str, samples: &[f32], sample_rate: u32) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::{Write, BufWriter};

    let mut file = BufWriter::new(File::create(path)?);

    let num_samples = samples.len();
    let byte_rate = sample_rate * 2; // 16-bit mono
    let data_size = num_samples * 2; // 2 bytes per sample (16-bit)
    let file_size = 36 + data_size;

    // WAVヘッダー
    file.write_all(b"RIFF")?;
    file.write_all(&(file_size as u32).to_le_bytes())?;
    file.write_all(b"WAVE")?;

    // fmtチャンク
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?; // チャンクサイズ
    file.write_all(&1u16.to_le_bytes())?;  // PCMフォーマット
    file.write_all(&1u16.to_le_bytes())?;  // チャンネル数（モノラル）
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;  // ブロックアライン
    file.write_all(&16u16.to_le_bytes())?; // ビット深度

    // dataチャンク
    file.write_all(b"data")?;
    file.write_all(&(data_size as u32).to_le_bytes())?;

    // オーディオデータ（16-bit PCM）
    for sample in samples {
        let sample_i16 = (*sample * 32767.0) as i16;
        file.write_all(&sample_i16.to_le_bytes())?;
    }

    file.flush()?;
    Ok(())
}