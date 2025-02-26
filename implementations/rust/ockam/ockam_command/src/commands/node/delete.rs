use clap::Args;

use super::util::{delete_all_nodes, delete_node};
use super::{default_node_name, HELP_DETAIL};
use crate::{help, CommandGlobalOpts};

/// Delete a node
#[derive(Clone, Debug, Args)]
#[command(arg_required_else_help = true, after_long_help = help::template(HELP_DETAIL))]
pub struct DeleteCommand {
    /// Name of the node.
    #[arg(default_value_t = default_node_name(), group = "nodes")]
    node_name: String,

    /// Terminate all node processes and delete all node configurations
    #[arg(long, short, group = "nodes")]
    all: bool,

    /// Terminate node process(es) immediately (uses SIGKILL instead of SIGTERM)
    #[arg(display_order = 901, long, short)]
    force: bool,
}

impl DeleteCommand {
    pub fn run(self, opts: CommandGlobalOpts) {
        if let Err(e) = run_impl(opts, self) {
            eprintln!("{e}");
            std::process::exit(e.code());
        }
    }
}

fn run_impl(opts: CommandGlobalOpts, cmd: DeleteCommand) -> crate::Result<()> {
    if cmd.all {
        delete_all_nodes(opts, cmd.force)?;
    } else {
        delete_node(&opts, &cmd.node_name, cmd.force)?;
        println!("Deleted node '{}'", &cmd.node_name);
    }
    Ok(())
}
