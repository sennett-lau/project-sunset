//! Native recording lifecycle. Model access remains in the core; this holds only transient audio
//! and a pinned speech destination. Cancel drops the in-flight HTTP future and its result.
use sunset_core::{PreparedSpeech, Sunset};
use tokio::sync::{Mutex, oneshot};

use crate::audio::{AudioCapture, CaptureStatus};

enum Session {
    Idle,
    Recording(PreparedSpeech),
    Transcribing(oneshot::Sender<()>),
}

struct State {
    generation: u64,
    session: Session,
}

pub struct VoiceState {
    capture: AudioCapture,
    state: Mutex<State>,
}

impl VoiceState {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            capture: AudioCapture::new()?,
            state: Mutex::new(State {
                generation: 0,
                session: Session::Idle,
            }),
        })
    }

    pub async fn start(&self, sunset: &Sunset) -> Result<(), String> {
        let mut state = self.state.lock().await;
        if !matches!(state.session, Session::Idle) {
            return Err("finish or cancel the current recording first".into());
        }
        let speech = sunset.prepare_speech().await.map_err(|e| e.to_string())?;
        self.capture.start().await?;
        state.generation = state.generation.wrapping_add(1);
        state.session = Session::Recording(speech);
        Ok(())
    }

    pub async fn stop(&self) -> Result<String, String> {
        let (speech, wav, generation, cancelled) = {
            let mut state = self.state.lock().await;
            if !matches!(state.session, Session::Recording(_)) {
                return Err("there is no recording to transcribe".into());
            }
            let Session::Recording(speech) = std::mem::replace(&mut state.session, Session::Idle)
            else {
                unreachable!()
            };
            let wav = self.capture.stop().await?;
            let (cancel, cancelled) = oneshot::channel();
            state.session = Session::Transcribing(cancel);
            (speech, wav, state.generation, cancelled)
        };
        let result = tokio::select! {
            _ = cancelled => Err("transcription cancelled".into()),
            result = speech.transcribe(wav) => result.map_err(|e| e.to_string()),
        };
        let mut state = self.state.lock().await;
        if state.generation != generation {
            return Err("transcription cancelled".into());
        }
        state.session = Session::Idle;
        result
    }

    pub async fn cancel(&self) -> Result<(), String> {
        let mut state = self.state.lock().await;
        state.generation = state.generation.wrapping_add(1);
        if let Session::Transcribing(cancel) = std::mem::replace(&mut state.session, Session::Idle)
        {
            let _ = cancel.send(());
        }
        self.capture.cancel().await
    }

    pub async fn status(&self) -> Result<CaptureStatus, String> {
        self.capture.status().await
    }
}
