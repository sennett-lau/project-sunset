//! Native, explicitly started microphone capture. The CPAL stream stays on one worker thread.
//! Audio exists only in bounded memory and leaves this module only when the caller stops capture.

use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use serde::Serialize;
use specta::Type;
use tokio::sync::oneshot;

const MAX_SECONDS: u32 = 120;
const OUTPUT_RATE: u32 = 16_000;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub recording: bool,
    pub elapsed_ms: u32,
    /// Capture reached the limit; `stop` can still retrieve the recording.
    pub auto_stopped: bool,
    pub error: Option<String>,
}

type Reply<T> = oneshot::Sender<Result<T, String>>;

enum Command {
    Start(Reply<()>),
    Stop(Reply<Vec<u8>>),
    Cancel(Reply<()>),
    Status(Reply<CaptureStatus>),
    Shutdown,
}

pub struct AudioCapture {
    commands: mpsc::Sender<Command>,
}

impl AudioCapture {
    /// Creating this service does not enumerate devices or request microphone access.
    pub fn new() -> Result<Self, String> {
        let (commands, incoming) = mpsc::channel();
        std::thread::Builder::new()
            .name("microphone-capture".into())
            .spawn(move || worker(incoming, Recording::start))
            .map_err(|e| format!("Could not start audio capture: {e}"))?;
        Ok(Self { commands })
    }

    pub async fn start(&self) -> Result<(), String> {
        self.request(Command::Start).await
    }

    /// Stops capture before encoding. Reaching the duration limit also stops the microphone,
    /// but retains its samples until this method retrieves them or `cancel` discards them.
    pub async fn stop(&self) -> Result<Vec<u8>, String> {
        self.request(Command::Stop).await
    }

    pub async fn cancel(&self) -> Result<(), String> {
        self.request(Command::Cancel).await
    }

    pub async fn status(&self) -> Result<CaptureStatus, String> {
        self.request(Command::Status).await
    }

    async fn request<T>(&self, command: impl FnOnce(Reply<T>) -> Command) -> Result<T, String> {
        let (reply, result) = oneshot::channel();
        self.commands
            .send(command(reply))
            .map_err(|_| "Audio capture is unavailable; restart the application".to_owned())?;
        tokio::time::timeout(COMMAND_TIMEOUT, result)
            .await
            .map_err(|_| "Microphone did not respond; check microphone permission in System Settings → Privacy & Security".to_owned())?
            .map_err(|_| "Audio capture stopped unexpectedly".to_owned())?
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
    }
}

struct Samples {
    mono: Vec<f32>,
    sample_rate: u32,
    capacity: usize,
    error: Option<String>,
}

impl Samples {
    fn new(sample_rate: u32) -> Result<Self, String> {
        if !(8_000..=96_000).contains(&sample_rate) {
            return Err(format!(
                "Unsupported microphone sample rate: {sample_rate} Hz"
            ));
        }
        let capacity = sample_rate as usize * MAX_SECONDS as usize;
        Ok(Self {
            mono: Vec::with_capacity(capacity),
            sample_rate,
            capacity,
            error: None,
        })
    }

    fn append<T>(&mut self, data: &[T], channels: usize)
    where
        T: Sample,
        f32: FromSample<T>,
    {
        if channels == 0 {
            return;
        }
        for frame in data.chunks_exact(channels) {
            if self.mono.len() == self.capacity {
                break;
            }
            let sample = frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels as f32;
            self.mono.push(if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                0.0
            });
        }
    }

    fn elapsed_ms(&self) -> u32 {
        (self.mono.len() as u64 * 1_000 / self.sample_rate as u64) as u32
    }

    fn wav(&self) -> Result<Vec<u8>, String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if self.mono.len() < self.sample_rate as usize / 5 {
            return Err(
                "Recording was too short; speak for at least a moment before stopping".into(),
            );
        }
        let power =
            self.mono.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / self.mono.len() as f64;
        if power < 1e-10 {
            return Err(
                "Recording was silent; check microphone permission and the selected input device"
                    .into(),
            );
        }
        Ok(encode_wav(&resample(&self.mono, self.sample_rate)))
    }
}

struct Recording {
    stream: Option<cpal::Stream>,
    samples: Arc<Mutex<Samples>>,
    started: Instant,
    auto_stopped: bool,
}

impl Recording {
    fn start() -> Result<Self, String> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or("No microphone is available; connect an input device")?;
        let supported = device
            .default_input_config()
            .map_err(|e| format!("Could not open the microphone: {e}"))?;
        let config = supported.config();
        if config.channels == 0 || config.channels > 32 {
            return Err("Unsupported microphone channel configuration".into());
        }
        let samples = Arc::new(Mutex::new(Samples::new(config.sample_rate.0)?));
        let stream = match supported.sample_format() {
            SampleFormat::F32 => input_stream::<f32>(&device, &config, samples.clone()),
            SampleFormat::F64 => input_stream::<f64>(&device, &config, samples.clone()),
            SampleFormat::I8 => input_stream::<i8>(&device, &config, samples.clone()),
            SampleFormat::I16 => input_stream::<i16>(&device, &config, samples.clone()),
            SampleFormat::I32 => input_stream::<i32>(&device, &config, samples.clone()),
            SampleFormat::I64 => input_stream::<i64>(&device, &config, samples.clone()),
            SampleFormat::U8 => input_stream::<u8>(&device, &config, samples.clone()),
            SampleFormat::U16 => input_stream::<u16>(&device, &config, samples.clone()),
            SampleFormat::U32 => input_stream::<u32>(&device, &config, samples.clone()),
            SampleFormat::U64 => input_stream::<u64>(&device, &config, samples.clone()),
            format => return Err(format!("Unsupported microphone format: {format}")),
        }?;
        stream.play().map_err(|e| {
            format!("Could not start the microphone; check microphone permission: {e}")
        })?;
        Ok(Self {
            stream: Some(stream),
            samples,
            started: Instant::now(),
            auto_stopped: false,
        })
    }

    fn tick(&mut self) {
        let samples = self.samples.lock().expect("audio samples");
        let limit = self.started.elapsed() >= Duration::from_secs(MAX_SECONDS.into())
            || samples.mono.len() == samples.capacity;
        let failed = samples.error.is_some();
        drop(samples);
        if self.stream.is_some() && (limit || failed) {
            self.stream.take();
            self.auto_stopped = limit;
        }
    }

    fn status(&self) -> CaptureStatus {
        let samples = self.samples.lock().expect("audio samples");
        CaptureStatus {
            recording: self.stream.is_some(),
            elapsed_ms: samples.elapsed_ms(),
            auto_stopped: self.auto_stopped,
            error: samples.error.clone(),
        }
    }

    fn finish(mut self) -> Result<Vec<u8>, String> {
        self.stream.take();
        self.samples.lock().expect("audio samples").wav()
    }
}

fn input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Samples>>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let errors = samples.clone();
    let channels = config.channels as usize;
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if let Ok(mut samples) = samples.lock() {
                    samples.append(data, channels);
                }
            },
            move |error| {
                if let Ok(mut samples) = errors.lock() {
                    samples.error = Some(format!("Microphone capture failed: {error}"));
                }
            },
            None,
        )
        .map_err(|e| format!("Could not open the microphone; check microphone permission: {e}"))
}

fn worker(incoming: mpsc::Receiver<Command>, mut start: impl FnMut() -> Result<Recording, String>) {
    let mut recording: Option<Recording> = None;
    loop {
        if let Some(recording) = &mut recording {
            recording.tick();
        }
        match incoming.recv_timeout(Duration::from_millis(50)) {
            Ok(Command::Start(reply)) => {
                let was_idle = recording.is_none();
                let result = if recording.is_some() {
                    Err("Finish or cancel the current recording first".into())
                } else if reply.is_closed() {
                    continue;
                } else {
                    start().map(|new| recording = Some(new))
                };
                // If the caller disappeared while permission was requested, release capture.
                if reply.send(result).is_err() && was_idle {
                    recording = None;
                }
            }
            Ok(Command::Stop(reply)) => {
                let result = recording
                    .take()
                    .ok_or_else(|| "There is no recording to transcribe".into())
                    .and_then(Recording::finish);
                let _ = reply.send(result);
            }
            Ok(Command::Cancel(reply)) => {
                recording = None;
                let _ = reply.send(Ok(()));
            }
            Ok(Command::Status(reply)) => {
                let _ = reply.send(Ok(recording
                    .as_ref()
                    .map(Recording::status)
                    .unwrap_or_default()));
            }
            Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

/// A short windowed-sinc low-pass filter prevents high-frequency input from folding into the
/// speech band while downsampling. Fractional kernels are cached for the whole recording.
fn resample(input: &[f32], sample_rate: u32) -> Vec<f32> {
    if sample_rate == OUTPUT_RATE {
        return input.to_vec();
    }
    const TAPS: usize = 64;
    const PHASES: usize = 256;
    let ratio = sample_rate as f64 / OUTPUT_RATE as f64;
    let cutoff = (1.0 / ratio).min(1.0) * 0.9;
    let kernels: Vec<[f64; TAPS]> = (0..PHASES)
        .map(|phase| {
            std::array::from_fn(|tap| {
                let offset = tap as f64 - (TAPS / 2 - 1) as f64 - phase as f64 / PHASES as f64;
                let x = std::f64::consts::PI * offset * cutoff;
                let sinc = if x.abs() < 1e-12 { 1.0 } else { x.sin() / x };
                let window = 0.5 + 0.5 * (std::f64::consts::PI * offset / (TAPS / 2) as f64).cos();
                cutoff * sinc * window
            })
        })
        .collect();
    let length = input.len() as u64 * OUTPUT_RATE as u64 / sample_rate as u64;
    (0..length as usize)
        .map(|index| {
            let position = index as f64 * ratio;
            let center = position.floor() as isize;
            let phase = (position.fract() * PHASES as f64) as usize;
            let mut value = 0.0;
            let mut weight = 0.0;
            for (tap, coefficient) in kernels[phase].iter().enumerate() {
                let source = center + tap as isize - (TAPS / 2 - 1) as isize;
                if let Some(sample) = usize::try_from(source).ok().and_then(|i| input.get(i)) {
                    value += *sample as f64 * coefficient;
                    weight += coefficient;
                }
            }
            (value / weight) as f32
        })
        .collect()
}

fn encode_wav(samples: &[f32]) -> Vec<u8> {
    let bytes = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + bytes as usize);
    wav.extend(b"RIFF");
    wav.extend((36 + bytes).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16_u32.to_le_bytes());
    wav.extend(1_u16.to_le_bytes()); // PCM
    wav.extend(1_u16.to_le_bytes()); // Mono
    wav.extend(OUTPUT_RATE.to_le_bytes());
    wav.extend((OUTPUT_RATE * 2).to_le_bytes());
    wav.extend(2_u16.to_le_bytes());
    wav.extend(16_u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(bytes.to_le_bytes());
    for sample in samples {
        let sample = sample.clamp(-1.0, 1.0);
        let scale = if sample < 0.0 { 32768.0 } else { 32767.0 };
        wav.extend(((sample * scale).round() as i16).to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_downmixes_sanitizes_and_stops_at_its_capacity() {
        let mut samples = Samples::new(16_000).unwrap();
        samples.capacity = 3;
        samples.append(&[1.0_f32, -1.0, 0.5, 0.5, f32::NAN, 0.0, 1.0, 1.0], 2);
        assert_eq!(samples.mono, [0.0, 0.5, 0.0]);
        samples.append(&[1.0_f32, 1.0], 2);
        assert_eq!(samples.mono.len(), 3);
    }

    #[test]
    fn rejects_silent_short_and_failed_recordings() {
        let mut samples = Samples::new(16_000).unwrap();
        samples.append(&[0.5_f32; 100], 1);
        assert!(samples.wav().unwrap_err().contains("too short"));
        samples.mono = vec![0.0; 16_000];
        assert!(samples.wav().unwrap_err().contains("silent"));
        samples.mono.fill(0.1);
        samples.error = Some("device disconnected".into());
        assert_eq!(samples.wav().unwrap_err(), "device disconnected");
    }

    #[test]
    fn resamples_duration_and_filters_aliasing_at_common_microphone_rates() {
        for rate in [8_000, 16_000, 44_100, 48_000, 96_000] {
            let input = vec![0.25; rate as usize];
            let output = resample(&input, rate);
            assert_eq!(output.len(), 16_000);
            assert!(output.iter().all(|sample| (*sample - 0.25).abs() < 1e-5));
        }
        let tone = |frequency: f64| {
            (0..48_000)
                .map(|i| (i as f64 * frequency * std::f64::consts::TAU / 48_000.0).sin() as f32)
                .collect::<Vec<_>>()
        };
        let rms = |samples: &[f32]| {
            (samples[100..15_900].iter().map(|s| s * s).sum::<f32>() / 15_800.0).sqrt()
        };
        assert!(rms(&resample(&tone(1_000.0), 48_000)) > 0.69);
        assert!(rms(&resample(&tone(12_000.0), 48_000)) < 0.01);
    }

    #[test]
    fn emits_valid_mono_pcm16_wav_with_clipped_extrema() {
        let wav = encode_wav(&[-2.0, -1.0, 0.0, 1.0, 2.0]);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 46);
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1);
        let decoded: Vec<i16> = wav[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| i16::from_le_bytes(*bytes))
            .collect();
        assert_eq!(decoded, [i16::MIN, i16::MIN, 0, i16::MAX, i16::MAX]);
    }

    #[tokio::test]
    async fn idle_service_status_stop_and_cancel_never_open_a_microphone() {
        let service = AudioCapture::new().unwrap();
        assert!(!service.status().await.unwrap().recording);
        assert!(service.stop().await.unwrap_err().contains("no recording"));
        service.cancel().await.unwrap();
        service.cancel().await.unwrap();
        assert!(service.status().await.unwrap().error.is_none());
    }

    fn synthetic_recording() -> Recording {
        let mut samples = Samples::new(16_000).unwrap();
        samples.append(&vec![0.25_f32; 16_000], 1);
        Recording {
            stream: None,
            samples: Arc::new(Mutex::new(samples)),
            started: Instant::now(),
            auto_stopped: false,
        }
    }

    #[tokio::test]
    async fn duplicate_start_preserves_capture_and_stop_consumes_it_once() {
        let (commands, incoming) = mpsc::channel();
        std::thread::spawn(move || worker(incoming, || Ok(synthetic_recording())));
        let service = AudioCapture { commands };
        service.start().await.unwrap();
        assert!(
            service
                .start()
                .await
                .unwrap_err()
                .contains("current recording")
        );
        assert_eq!(service.status().await.unwrap().elapsed_ms, 1_000);
        assert_eq!(service.stop().await.unwrap().len(), 32_044);
        assert!(service.stop().await.is_err());
        service.start().await.unwrap();
        service.cancel().await.unwrap();
        assert!(service.stop().await.is_err());
    }

    #[tokio::test]
    async fn caller_disappearing_during_start_releases_the_recording() {
        let (commands, incoming) = mpsc::channel();
        let (starting, started) = oneshot::channel();
        let mut starting = Some(starting);
        let (release, gate) = mpsc::channel();
        std::thread::spawn(move || {
            worker(incoming, || {
                let _ = starting.take().unwrap().send(());
                gate.recv().unwrap();
                Ok(synthetic_recording())
            });
        });
        let service = AudioCapture { commands };
        let (reply, result) = oneshot::channel();
        service.commands.send(Command::Start(reply)).unwrap();
        started.await.unwrap();
        drop(result);
        release.send(()).unwrap();
        assert_eq!(service.status().await.unwrap().elapsed_ms, 0);
        assert!(service.stop().await.is_err());
    }
}
