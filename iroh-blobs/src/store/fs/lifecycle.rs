//! 文件 store 的唯一 runtime 所有者与完整关闭确认。

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
};

use n0_error::{Result, StdResultExt};
use n0_future::future::block_on;
use tokio::{
    runtime::Builder,
    sync::{mpsc, oneshot},
};

use super::{options::Options, Actor, FsStore};
use crate::{api::Store, store::gc::run_gc_with_factory};

pub(super) async fn load(db_path: PathBuf, options: Options) -> Result<FsStore> {
    static THREAD_NR: AtomicU64 = AtomicU64::new(0);
    let (commands_tx, commands_rx) = mpsc::channel(100);
    let (fs_commands_tx, fs_commands_rx) = mpsc::channel(100);
    let internal_tx = fs_commands_tx.clone();
    let weak_commands = commands_tx.downgrade();
    let (ready_tx, ready_rx) = oneshot::channel();

    // runtime 必须在其工作线程之外创建和销毁，不能在自身 worker 上等待自身退出。
    let _owner = thread::Builder::new()
        .name("iroh-blob-owner".into())
        .spawn(move || {
            let runtime = match Builder::new_multi_thread()
                .thread_name_fn(|| {
                    format!(
                        "iroh-blob-store-{}",
                        THREAD_NR.fetch_add(1, Ordering::Relaxed)
                    )
                })
                .enable_time()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready_tx.send(Err(error.into()));
                    return;
                }
            };
            let (shutdowns, succeeded) = runtime.block_on(async move {
                let gc_config = options.gc.clone();
                let actor = match Actor::new(
                    db_path,
                    commands_rx,
                    fs_commands_rx,
                    internal_tx,
                    Arc::new(options),
                )
                .await
                {
                    Ok(actor) => actor,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return (Vec::new(), false);
                    }
                };
                let gc = gc_config.map(|config| {
                    tokio::spawn(run_gc_with_factory(
                        move || {
                            weak_commands
                                .upgrade()
                                .map(|sender| Store::from_sender(sender.into()))
                        },
                        config,
                    ))
                });
                let _ = ready_tx.send(Ok(()));
                actor.run(gc).await
            });
            drop(runtime);
            // 确认不依赖加载时的调用方 runtime；私有 runtime 已完整退出。
            // 失败也持有原回应通道至这里，不能提前报告失败而遗弃收尾。
            if succeeded {
                block_on(async move {
                    for cmd in shutdowns {
                        let _ = cmd.tx.send(()).await;
                    }
                });
            }
        })?;

    ready_rx.await.anyerr()??;
    Ok(FsStore::new(commands_tx.into(), fs_commands_tx))
}
