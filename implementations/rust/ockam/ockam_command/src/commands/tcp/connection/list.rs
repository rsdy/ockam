use anyhow::Context;
use clap::Args;
use cli_table::{print_stdout, Cell, Style, Table};
use ockam_api::nodes::models;
use ockam_api::nodes::models::transport::TransportStatus;
use ockam_core::api::Request;

use crate::commands::node::NodeOpts;
use crate::util::{extract_address_value, node_rpc, Rpc};
use crate::CommandGlobalOpts;

#[derive(Args, Clone, Debug)]
pub struct ListCommand {
    #[command(flatten)]
    node_opts: NodeOpts,
}

impl ListCommand {
    pub fn run(self, options: CommandGlobalOpts) {
        node_rpc(run_impl, (options, self))
    }
}

async fn run_impl(
    ctx: ockam::Context,
    (options, command): (CommandGlobalOpts, ListCommand),
) -> crate::Result<()> {
    let node_name = extract_address_value(&command.node_opts.api_node)?;
    let mut rpc = Rpc::background(&ctx, &options, &node_name)?;
    rpc.request(Request::get("/node/tcp/connection")).await?;
    let response = rpc.parse_response::<models::transport::TransportList>()?;

    let table = response
        .list
        .iter()
        .fold(
            vec![],
            |mut acc,
             TransportStatus {
                 tt,
                 tm,
                 payload,
                 tid,
                 ..
             }| {
                let row = vec![tid.cell(), tt.cell(), tm.cell(), payload.cell()];
                acc.push(row);
                acc
            },
        )
        .table()
        .title(vec![
            "Transport ID".cell().bold(true),
            "Transport Type".cell().bold(true),
            "Mode".cell().bold(true),
            "Address bind".cell().bold(true),
        ]);

    print_stdout(table).context("failed to print node status")?;
    Ok(())
}
