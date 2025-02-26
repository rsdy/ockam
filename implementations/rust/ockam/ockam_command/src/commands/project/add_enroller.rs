use clap::Args;
use ockam::Context;
use ockam_api::cloud::project::Enroller;

use crate::commands::node::util::delete_embedded_node;
use crate::util::api::{self, CloudOpts};
use crate::util::{node_rpc, Rpc};
use crate::{help, CommandGlobalOpts};

/// Adds an authorized enroller to the project' authority
#[derive(Clone, Debug, Args)]
#[command(hide = help::hide())]
pub struct AddEnrollerCommand {
    /// Id of the project.
    #[arg(display_order = 1001)]
    pub project_id: String,

    /// Identity id to add as an authorized enroller.
    #[arg(display_order = 1002)]
    pub enroller_identity_id: String,

    /// Description of this enroller, optional.
    #[arg(display_order = 1003)]
    pub description: Option<String>,

    #[command(flatten)]
    pub cloud_opts: CloudOpts,
}

impl AddEnrollerCommand {
    pub fn run(self, options: CommandGlobalOpts) {
        node_rpc(rpc, (options, self));
    }
}

async fn rpc(
    mut ctx: Context,
    (opts, cmd): (CommandGlobalOpts, AddEnrollerCommand),
) -> crate::Result<()> {
    run_impl(&mut ctx, opts, cmd).await
}

async fn run_impl(
    ctx: &mut Context,
    opts: CommandGlobalOpts,
    cmd: AddEnrollerCommand,
) -> crate::Result<()> {
    let mut rpc = Rpc::embedded(ctx, &opts).await?;
    rpc.request(api::project::add_enroller(&cmd)).await?;
    rpc.parse_and_print_response::<Enroller>()?;
    delete_embedded_node(&opts, rpc.node_name()).await;
    Ok(())
}
