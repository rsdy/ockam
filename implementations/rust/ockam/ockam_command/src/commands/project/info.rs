use anyhow::Context as _;
use clap::Args;
use ockam::Context;
use ockam_api::cloud::project::Project;

use crate::commands::node::util::{delete_embedded_node, start_embedded_node};
use crate::commands::project::util::config;
use crate::config::project::*;
use crate::util::api::{self, CloudOpts};
use crate::util::{node_rpc, RpcBuilder};
use crate::CommandGlobalOpts;

#[derive(Clone, Debug, Args)]
pub struct InfoCommand {
    /// Name of the project.
    #[arg(default_value = "default")]
    pub name: String,

    #[command(flatten)]
    pub cloud_opts: CloudOpts,
}

impl InfoCommand {
    pub fn run(self, options: CommandGlobalOpts) {
        node_rpc(rpc, (options, self));
    }
}

async fn rpc(mut ctx: Context, (opts, cmd): (CommandGlobalOpts, InfoCommand)) -> crate::Result<()> {
    run_impl(&mut ctx, opts, cmd).await
}

async fn run_impl(
    ctx: &mut Context,
    opts: CommandGlobalOpts,
    cmd: InfoCommand,
) -> crate::Result<()> {
    let controller_route = &cmd.cloud_opts.route();
    let node_name = start_embedded_node(ctx, &opts, None).await?;

    // Lookup project
    let id = match config::get_project(&opts.config, &cmd.name) {
        Some(id) => id,
        None => {
            config::refresh_projects(ctx, &opts, &node_name, &cmd.cloud_opts.route(), None).await?;
            config::get_project(&opts.config, &cmd.name)
                .context(format!("Project '{}' does not exist", cmd.name))?
        }
    };

    // Send request
    let mut rpc = RpcBuilder::new(ctx, &opts, &node_name).build();
    rpc.request(api::project::show(&id, controller_route))
        .await?;
    let info: ProjectInfo = rpc.parse_response::<Project>()?.into();
    rpc.print_response(&info)?;
    delete_embedded_node(&opts, rpc.node_name()).await;
    Ok(())
}
