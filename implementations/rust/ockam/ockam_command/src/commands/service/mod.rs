pub(crate) mod config;
pub(crate) mod list;
pub(crate) mod start;
pub(crate) mod util;

use clap::{Args, Subcommand};
use list::ListCommand;
pub(crate) use start::StartCommand;

use crate::{help, CommandGlobalOpts};

#[derive(Clone, Debug, Args)]
#[command(hide = help::hide())]
pub struct ServiceCommand {
    #[command(subcommand)]
    subcommand: ServiceSubcommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum ServiceSubcommand {
    #[command(display_order = 900)]
    Start(StartCommand),
    #[command(display_order = 901)]
    List(ListCommand),
}

impl ServiceCommand {
    pub fn run(self, options: CommandGlobalOpts) {
        match self.subcommand {
            ServiceSubcommand::Start(c) => c.run(options),
            ServiceSubcommand::List(c) => c.run(options),
        }
    }
}
