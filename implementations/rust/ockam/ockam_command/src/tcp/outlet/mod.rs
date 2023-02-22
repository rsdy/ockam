mod create;

use clap::{Args, Subcommand};
use create::CreateCommand;

use crate::CommandGlobalOpts;

/// Manage TCP Outlets
#[derive(Clone, Debug, Args)]
pub struct TcpOutletCommand {
    #[command(subcommand)]
    subcommand: TcpOutletSubCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum TcpOutletSubCommand {
    Create(CreateCommand),
}

impl TcpOutletCommand {
    pub fn run(self, options: CommandGlobalOpts) {
        match self.subcommand {
            TcpOutletSubCommand::Create(c) => c.run(options),
        }
    }
}
