//! Persistence scenarios use synthetic Bao imports and real filesystem storage.
//! The retained-context case deliberately isolates the state-owner invariant.

use std::{fs, time::Duration};

use n0_future::{stream, StreamExt};
use serde_json::json;
use testresult::TestResult;
use tokio::{sync::oneshot as control, time::timeout};

use super::{
    options::{InlineOptions, Options},
    tests::{create_n0_bao_full, test_data},
    Actor, BaoFileStorage, EntityApi, FsStore, SpawnArg,
};
use crate::protocol::ChunkRangesExt;
use bao_tree::ChunkRanges;
use bytes::Bytes;

fn evidence(name: &str, value: serde_json::Value) -> TestResult<()> {
    if let Some(root) = std::env::var_os("BLOB_PERSISTENCE_EVIDENCE") {
        fs::create_dir_all(&root)?;
        fs::write(
            std::path::PathBuf::from(root).join(format!("{name}.json")),
            serde_json::to_vec_pretty(&value)?,
        )?;
    }
    Ok(())
}

async fn retained_context(fail_flush: bool) -> TestResult<()> {
    let dir = tempfile::tempdir()?;
    let mut options = Options::new(dir.path());
    options.inline = InlineOptions::NO_INLINE;
    let data = test_data(128 * 1024);
    let (hash, ranges, encoded) = create_n0_bao_full(&data, &ChunkRanges::last_chunk())?;
    let bitfield_path = options.path.bitfield_path(&hash);
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let (internal_tx, internal_rx) = tokio::sync::mpsc::channel(100);
    let mut actor = Actor::new(
        dir.path().join("blobs.db"),
        rx,
        internal_rx,
        internal_tx.clone(),
        options.into(),
    )
    .await?;
    let (captured, context) = control::channel();
    let (release, held) = control::channel();
    let task = actor
        .handles
        .spawn(hash, |arg| async move {
            if let SpawnArg::Active(ctx) = arg {
                let _ = captured.send(ctx);
                let _ = held.await;
            }
        })
        .await
        .ok_or("entity task missing")?;
    actor.tasks.spawn(task);
    let store = FsStore::new(tx.into(), internal_tx);
    let running = tokio::spawn(actor.run(None));
    let ctx = timeout(Duration::from_secs(5), context).await??;
    store
        .import_bao_bytes(hash, ranges.clone(), Bytes::from(encoded))
        .await?;
    let before = store.observe(hash).await?;
    assert_eq!(before.ranges, ranges);
    let before_state = match &*ctx.state.borrow() {
        BaoFileStorage::Partial(_) => "Partial",
        _ => return Err("fixture did not reach disk-backed Partial".into()),
    };
    if fail_flush {
        fs::create_dir(&bitfield_path)?;
    }
    // Call the actual state-owner operation while retaining this entity generation.
    // Idle recycling normally resets a generation before queued commands reuse it.
    let _ = ctx.persist().await;
    let after_state = match &*ctx.state.borrow() {
        BaoFileStorage::Partial(_) => "Partial",
        BaoFileStorage::Poisoned => "Poisoned",
        _ => "Other",
    };
    let observed = timeout(Duration::from_secs(5), store.observe(hash)).await?;
    let observed_ok = observed.as_ref().is_ok_and(|v| v.ranges == before.ranges);
    evidence(
        if fail_flush {
            "retained-failed-flush"
        } else {
            "retained-successful-flush"
        },
        json!({
            "before_state": before_state, "after_state": after_state,
            "flush_path_is_file": bitfield_path.is_file(), "injected_flush_failure": fail_flush,
            "reobserve_preserved_ranges": observed_ok,
            "boundary": "retained entity context; not an iOS or normal idle-recycle reproduction"
        }),
    )?;
    if fail_flush {
        fs::remove_dir(&bitfield_path)?;
    }
    let _ = release.send(());
    drop(ctx);
    // This fixture runs the real actor on the test runtime, without its thread owner.
    let closing = timeout(Duration::from_secs(5), store.shutdown());
    let (response, joined) = tokio::join!(closing, running);
    let _ = response;
    joined?;
    assert!(
        observed_ok,
        "post-persistence observation lost its valid state: {after_state}"
    );
    Ok(())
}

#[tokio::test]
async fn partial_successful_flush_preserves_observation() -> TestResult<()> {
    retained_context(false).await
}

#[tokio::test]
async fn partial_failed_flush_preserves_observation() -> TestResult<()> {
    retained_context(true).await
}

async fn public_shutdown(fail_flush: bool) -> TestResult<()> {
    let dir = tempfile::tempdir()?;
    let mut options = Options::new(dir.path());
    options.inline = InlineOptions::NO_INLINE;
    let data = test_data(128 * 1024);
    let (hash, ranges, encoded) = create_n0_bao_full(&data, &ChunkRanges::last_chunk())?;
    let bitfield_path = options.path.bitfield_path(&hash);
    let store = FsStore::load_with_opts(dir.path().join("blobs.db"), options.clone()).await?;
    // Subscribe before writing so the partial entity stays active until shutdown.
    let mut observer = Box::pin(store.observe(hash).stream().await?);
    let _ = timeout(Duration::from_secs(5), observer.next())
        .await?
        .ok_or("initial observation missing")?;
    store
        .import_bao_bytes(hash, ranges.clone(), Bytes::from(encoded))
        .await?;
    loop {
        let next = timeout(Duration::from_secs(5), observer.next())
            .await?
            .ok_or("partial observation missing")?;
        if next.ranges == ranges {
            break;
        }
    }
    if fail_flush {
        fs::create_dir(&bitfield_path)?;
    }
    let closed = timeout(Duration::from_secs(5), store.shutdown()).await?;
    let observer_ended = timeout(Duration::from_secs(5), observer.next())
        .await?
        .is_none();
    let correctly_classified = closed.is_err() == fail_flush;
    evidence(
        if fail_flush {
            "shutdown-failed-flush"
        } else {
            "shutdown-successful-flush"
        },
        json!({
            "injected_flush_failure": fail_flush, "shutdown_ok": closed.is_ok(),
            "observer_ended": observer_ended, "flush_path_is_file": bitfield_path.is_file(),
            "correctly_classified": correctly_classified
        }),
    )?;
    assert!(observer_ended);
    assert!(
        correctly_classified,
        "shutdown falsely acknowledged failed persistence"
    );
    if fail_flush {
        fs::remove_dir(&bitfield_path)?;
    }
    // A completed shutdown must release the database, even with client clones alive.
    let reopened = FsStore::load_with_opts(dir.path().join("blobs.db"), options).await?;
    let actual = reopened.observe(hash).await?;
    assert_eq!(actual.ranges, ranges);
    let (hash, all, encoded) = create_n0_bao_full(&data, &ChunkRanges::all())?;
    reopened
        .import_bao_bytes(hash, all, Bytes::from(encoded))
        .await?;
    assert_eq!(reopened.get_bytes(hash).await?, data);
    timeout(Duration::from_secs(5), reopened.shutdown()).await??;
    evidence(
        if fail_flush {
            "recovery-failed-flush"
        } else {
            "recovery-successful-flush"
        },
        json!({
            "partial_ranges_recovered": true, "completed_bytes_exact": true, "database_reopened": true
        }),
    )?;
    Ok(())
}

#[tokio::test]
async fn partial_successful_shutdown_reopens_and_completes() -> TestResult<()> {
    public_shutdown(false).await
}

#[tokio::test]
async fn partial_failed_flush_must_not_acknowledge_shutdown() -> TestResult<()> {
    public_shutdown(true).await
}

#[tokio::test]
async fn held_import_deadline_keeps_store_open_until_completion() -> TestResult<()> {
    let dir = tempfile::tempdir()?;
    let store = FsStore::load(dir.path()).await?;
    let importer = store.clone();
    let closer = store.clone();
    let (started, entered) = control::channel();
    let (release, resume) = control::channel();
    let importing = tokio::spawn(async move {
        let input = stream::unfold(Some((started, resume)), |state| async move {
            let (started, resume) = state?;
            let _ = started.send(());
            let item = resume
                .await
                .map_err(std::io::Error::other)
                .map(|()| Bytes::from_static(b"synthetic held import"));
            Some((item, None))
        });
        importer.add_stream(input).await.temp_tag().await
    });
    timeout(Duration::from_secs(5), entered).await??;
    let mut closing = tokio::spawn(async move { closer.shutdown().await });
    let deadline_expired = timeout(Duration::from_millis(50), &mut closing)
        .await
        .is_err();
    // redb's normal open provides a cross-instance database-lock check.
    let db_blocked = FsStore::load(dir.path()).await.is_err();
    let _ = release.send(());
    let tag = timeout(Duration::from_secs(5), importing).await???;
    timeout(Duration::from_secs(5), closing).await???;
    let reopened = FsStore::load(dir.path()).await?;
    assert_eq!(
        reopened.get_bytes(tag.hash()).await?.as_ref(),
        b"synthetic held import"
    );
    timeout(Duration::from_secs(5), reopened.shutdown()).await??;
    evidence(
        "held-import",
        json!({
            "deadline_expired": deadline_expired, "database_open_blocked_during_cleanup": db_blocked,
            "database_reopened_after_completion": true, "accepted_import_bytes_exact": true
        }),
    )?;
    assert!(deadline_expired && db_blocked);
    Ok(())
}
