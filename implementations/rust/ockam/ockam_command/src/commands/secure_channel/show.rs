use clap::Args;
use ockam::Context;
use ockam_api::nodes::models::secure_channel::ShowSecureChannelResponse;
use ockam_core::Address;

use crate::commands::secure_channel::HELP_DETAIL;
use crate::util::{api, extract_address_value, node_rpc, Rpc};
use crate::{help, CommandGlobalOpts, Result};

/// Show Secure Channels
#[derive(Clone, Debug, Args)]
#[command(arg_required_else_help = true, after_long_help = help::template(HELP_DETAIL))]
pub struct ShowCommand {
    /// Node
    #[arg(value_name = "NODE", long, display_order = 800)]
    at: String,

    /// Channel address
    #[arg(display_order = 800)]
    address: Address,
}

impl ShowCommand {
    pub fn run(self, options: CommandGlobalOpts) {
        node_rpc(rpc, (options, self));
    }

    // Read the `at` argument and return node name
    fn parse_at_node(&self) -> String {
        extract_address_value(&self.at).unwrap_or_else(|_| "".to_string())
    }
}

async fn rpc(ctx: Context, (options, command): (CommandGlobalOpts, ShowCommand)) -> Result<()> {
    let at = &command.parse_at_node();
    let address = &command.address;

    let mut rpc = Rpc::background(&ctx, &options, at)?;
    let request = api::show_secure_channel(address);
    rpc.request(request).await?;
    let response = rpc.parse_response::<ShowSecureChannelResponse>()?;

    rpc.print_response(response)?;

    Ok(())
}
