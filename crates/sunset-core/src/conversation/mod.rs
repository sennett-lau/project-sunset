//! The fast path: store the user turn, stream the tutor answer, hand off to curation.

use futures::StreamExt;
use sunset_domain::{AppEvent, SendMessageResult, SessionId, Speaker, Turn, TurnId};
use sunset_providers::{ChatMessage, ChatRequest};
use sunset_store::repo::sessions;

use crate::{CoreError, Result, Sunset, jobs};

const TUTOR_PROMPT: &str = include_str!("../prompts/tutor.md");
/// How many earlier turns the tutor sees. Retrieval-based context arrives in Phase 2.
const HISTORY_TURNS: usize = 20;
const MAX_MESSAGE_CHARS: usize = 20_000;

pub(crate) async fn send_message(
    app: &Sunset,
    session_id: Option<SessionId>,
    text: &str,
) -> Result<SendMessageResult> {
    let text = text.trim();
    if text.is_empty() {
        return Err(CoreError::Invalid("message is empty".into()));
    }
    if text.chars().count() > MAX_MESSAGE_CHARS {
        return Err(CoreError::Invalid(format!(
            "message is longer than {MAX_MESSAGE_CHARS} characters"
        )));
    }
    let inner = &app.inner;
    let mut conn = inner.store.conn().await?;
    let session_id = match session_id {
        Some(id) if sessions::exists(&mut conn, id).await? => id,
        Some(_) => return Err(CoreError::NotFound("session")),
        None => sessions::create(&mut conn, inner.user).await?.id,
    };
    let user_turn =
        sessions::insert_turn(&mut conn, TurnId::new(), session_id, Speaker::User, text).await?;
    let history = sessions::list_turns(&mut conn, session_id).await?;
    drop(conn);
    inner.bus.emit(AppEvent::TurnFinalized {
        turn: user_turn.clone(),
    });

    let tutor_turn_id = TurnId::new();
    let task_app = app.clone();
    let task_user_turn = user_turn.clone();
    tokio::spawn(async move {
        if let Err(e) = answer(&task_app, &history, &task_user_turn, tutor_turn_id).await {
            tracing::warn!(error = %e, "tutor answer failed");
            task_app.inner.bus.emit(AppEvent::TutorFailed {
                session_id: task_user_turn.session_id,
                turn_id: tutor_turn_id,
                message: e.to_string(),
            });
        }
    });

    Ok(SendMessageResult {
        session_id,
        user_turn,
        tutor_turn_id,
    })
}

/// Stream the answer, store it, then queue curation.
async fn answer(app: &Sunset, history: &[Turn], user_turn: &Turn, turn_id: TurnId) -> Result<()> {
    let inner = &app.inner;
    let request = tutor_request(history);
    let mut stream = inner.providers.tutor.stream(request).await?;
    let mut full = String::new();
    while let Some(delta) = stream.next().await {
        let delta = delta?;
        full.push_str(&delta.text);
        inner.bus.emit(AppEvent::TutorDelta {
            session_id: user_turn.session_id,
            turn_id,
            text: delta.text,
        });
    }
    let mut conn = inner.store.conn().await?;
    let tutor_turn = sessions::insert_turn(
        &mut conn,
        turn_id,
        user_turn.session_id,
        Speaker::Tutor,
        &full,
    )
    .await?;
    drop(conn);
    inner.bus.emit(AppEvent::TurnFinalized {
        turn: tutor_turn.clone(),
    });
    jobs::queue_curation(app, user_turn, &tutor_turn).await
}

fn tutor_request(history: &[Turn]) -> ChatRequest {
    let start = history.len().saturating_sub(HISTORY_TURNS);
    let mut messages = vec![ChatMessage::system(TUTOR_PROMPT)];
    messages.extend(history[start..].iter().map(|t| match t.speaker {
        Speaker::User => ChatMessage::user(&t.text),
        Speaker::Tutor => ChatMessage::assistant(&t.text),
    }));
    ChatRequest {
        messages,
        ..Default::default()
    }
}
