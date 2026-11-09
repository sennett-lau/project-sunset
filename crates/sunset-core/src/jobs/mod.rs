//! Durable background jobs. Each job is a row in `jobs`; unfinished rows run again on startup.
//! Phase 0 has one kind, `curate_turn`.

use serde::{Deserialize, Serialize};
use sunset_domain::{
    AppEvent, JobId, Proposal, ProposalId, ProposalStatus, Role, SaveMode, Turn, TurnId,
};
use sunset_store::repo::{jobs, knowledge as kn, proposals, sessions, users};

use crate::knowledge::{apply, curator, validate};
use crate::{CoreError, Result, Sunset, models};

const CURATE_TURN: &str = "curate_turn";
const MAX_ATTEMPTS: i64 = 3;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurateTurn {
    user_turn_id: TurnId,
    tutor_turn_id: TurnId,
}

pub(crate) async fn queue_curation(
    app: &Sunset,
    user_turn: &Turn,
    tutor_turn: &Turn,
) -> Result<()> {
    let payload = serde_json::to_value(CurateTurn {
        user_turn_id: user_turn.id,
        tutor_turn_id: tutor_turn.id,
    })
    .map_err(|e| CoreError::Invalid(e.to_string()))?;
    let key = format!("{CURATE_TURN}:{}", tutor_turn.id);
    let mut conn = app.inner.store.conn().await?;
    let id = jobs::enqueue(&mut conn, CURATE_TURN, &payload, &key).await?;
    drop(conn);
    spawn(app, id);
    Ok(())
}

/// Re-run jobs left unfinished by the last run of the app.
pub(crate) async fn resume(app: &Sunset) -> Result<()> {
    let ids = jobs::unfinished(&mut *app.inner.store.conn().await?).await?;
    for id in ids {
        spawn(app, id);
    }
    Ok(())
}

fn spawn(app: &Sunset, id: JobId) {
    let app = app.clone();
    tokio::spawn(async move {
        if let Err(e) = run(&app, id).await {
            tracing::warn!(job = %id, error = %e, "job failed");
            if let Ok(mut conn) = app.inner.store.conn().await {
                let _ = jobs::finish(&mut conn, id, Some(&e.to_string())).await;
            }
            app.inner.bus.emit(AppEvent::JobFailed {
                job_id: id,
                message: e.to_string(),
            });
        }
    });
}

async fn run(app: &Sunset, id: JobId) -> Result<()> {
    let inner = &app.inner;
    let Some(job) = jobs::start(&mut *inner.store.conn().await?, id).await? else {
        return Ok(()); // already finished
    };
    if job.attempts > MAX_ATTEMPTS {
        return Err(CoreError::Conflict(format!(
            "gave up after {MAX_ATTEMPTS} attempts"
        )));
    }
    match job.kind.as_str() {
        CURATE_TURN => {
            let payload: CurateTurn = serde_json::from_value(job.payload)
                .map_err(|e| CoreError::Invalid(e.to_string()))?;
            curate(app, id, payload).await
        }
        other => Err(CoreError::Invalid(format!("unknown job kind {other:?}"))),
    }
}

/// Curator → validation → save policy. Proposals and the job's completion are written in one
/// transaction, so a crash in between re-runs the job without duplicating proposals.
async fn curate(app: &Sunset, job: JobId, payload: CurateTurn) -> Result<()> {
    let inner = &app.inner;
    let mut conn = inner.store.conn().await?;
    let user_turn = sessions::get_turn(&mut conn, payload.user_turn_id)
        .await?
        .ok_or(CoreError::NotFound("turn"))?;
    let tutor_turn = sessions::get_turn(&mut conn, payload.tutor_turn_id)
        .await?
        .ok_or(CoreError::NotFound("turn"))?;
    let existing = kn::live_concepts(&mut conn, inner.user).await?;
    let save_mode = users::save_mode(&mut conn, inner.user).await?;
    drop(conn);

    let model = models::model_for(app, Role::Curator).await?;
    let output = curator::propose(model.as_ref(), &user_turn, &tutor_turn, &existing).await?;

    let names = validate::NameIndex::new(&existing);
    let mut created = Vec::new();
    for mut draft in output.proposals {
        names.normalize(&mut draft);
        if let Err(reason) = validate::draft(&draft, &names) {
            tracing::warn!(%reason, summary = %draft.summary, "dropped invalid proposal");
            continue;
        }
        created.push(Proposal {
            id: ProposalId::new(),
            session_id: user_turn.session_id,
            evidence_turn_ids: vec![user_turn.id, tutor_turn.id],
            summary: draft.summary,
            changes: draft.changes,
            status: ProposalStatus::AwaitingApproval,
            revision: 0,
            article_base: None,
            commit_id: None,
            created_at: chrono_now(),
        });
    }

    let mut tx = inner.store.begin().await?;
    for p in &created {
        proposals::insert(&mut tx, inner.user, p).await?;
    }
    jobs::finish(&mut tx, job, None).await?;
    tx.commit().await?;

    for p in created {
        let id = p.id;
        inner.bus.emit(AppEvent::ProposalCreated { proposal: p });
        if save_mode == SaveMode::Auto {
            apply::auto_commit(app, id).await?;
        }
    }
    Ok(())
}

fn chrono_now() -> sunset_domain::Timestamp {
    sunset_domain::Timestamp::from(std::time::SystemTime::now())
}
