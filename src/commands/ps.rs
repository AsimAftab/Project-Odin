use anyhow::Result;

use crate::cli::PsArgs;
use crate::ui::process_dashboard;

pub async fn run(_ctx: crate::core::context::AppContext, _args: PsArgs) -> Result<()> {
    process_dashboard::run().await?;
    Ok(())
}
