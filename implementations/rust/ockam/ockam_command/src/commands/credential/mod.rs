pub(crate) mod get_credential;
pub(crate) mod present_credential;

use clap::{Args, Subcommand};
pub(crate) use get_credential::GetCredentialCommand;
pub(crate) use present_credential::PresentCredentialCommand;

use crate::{help, CommandGlobalOpts};

const HELP_DETAIL: &str = "";

#[derive(Clone, Debug, Args)]
#[command(
    hide = help::hide(),
    after_long_help = help::template(HELP_DETAIL),
    arg_required_else_help = true,
    subcommand_required = true
)]
pub struct CredentialCommand {
    #[command(subcommand)]
    subcommand: CredentialSubcommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum CredentialSubcommand {
    Get(GetCredentialCommand),
    Present(PresentCredentialCommand),
}

impl CredentialCommand {
    pub fn run(self, options: CommandGlobalOpts) {
        match self.subcommand {
            CredentialSubcommand::Get(c) => c.run(options),
            CredentialSubcommand::Present(c) => c.run(options),
        }
    }
}
