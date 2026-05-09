use anyhow::Result;

use crate::cli::RestoreArgs;
use crate::core::context::AppContext;
use crate::services::{restore_service::RestoreService, storage::SnapshotStore};

pub async fn run(ctx: AppContext, args: RestoreArgs) -> Result<()> {
    RestoreService::new(SnapshotStore::new(ctx.odin_dir().clone()))
        .restore(args.apply, args.continue_on_error)
        .await
}
