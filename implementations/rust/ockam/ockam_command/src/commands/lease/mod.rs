mod create;
mod list;
mod revoke;
mod show;

use clap::{Args, Subcommand};
pub use create::CreateCommand;
pub use list::ListCommand;
pub use show::ShowCommand;

use self::revoke::RevokeCommand;
use crate::util::api::{CloudOpts, ProjectOpts};
use crate::CommandGlobalOpts;

#[derive(Clone, Debug, Args)]
#[command(arg_required_else_help = true, subcommand_required = true)]
pub struct LeaseCommand {
    #[command(subcommand)]
    subcommand: LeaseSubcommand,

    #[command(flatten)]
    cloud_opts: CloudOpts,

    #[command(flatten)]
    project_opts: ProjectOpts,
}

#[derive(Clone, Debug, Subcommand)]
pub enum LeaseSubcommand {
    Create(CreateCommand),
    List(ListCommand),
    Show(ShowCommand),
    Revoke(RevokeCommand),
}

const TOKEN_VIEW: &str = r#"
### Token
> **ID:** ${id}
> **Issued For:** ${issued_for}
> **Created At:** ${created_at}
> **Expires At:** ${expires_at}
> **Token:** ${token}
> **Status:** ${status}
"#;

impl LeaseCommand {
    pub fn run(self, options: CommandGlobalOpts) {
        match self.subcommand {
            LeaseSubcommand::Create(c) => c.run(options, self.cloud_opts, self.project_opts),
            LeaseSubcommand::List(c) => c.run(options, self.cloud_opts, self.project_opts),
            LeaseSubcommand::Show(c) => c.run(options, self.cloud_opts, self.project_opts),
            LeaseSubcommand::Revoke(c) => c.run(options, self.cloud_opts, self.project_opts),
        }
    }
}
