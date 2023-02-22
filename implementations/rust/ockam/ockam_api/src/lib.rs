pub mod auth;
pub mod authenticator;
pub mod bootstrapped_identities_store;
pub mod cli_state;
pub mod cloud;
pub mod config;
pub mod echoer;
pub mod error;
pub mod hop;
pub mod identity;
pub mod kafka;
pub mod nodes;
pub mod okta;
pub mod port_range;
pub mod uppercase;
pub mod vault;
pub mod verifier;

mod multiaddr;
mod session;
pub use multiaddr::*;

#[cfg(feature = "lmdb")]
pub mod lmdb;

#[macro_use]
extern crate tracing;

pub struct DefaultAddress;

impl DefaultAddress {
    pub const VAULT_SERVICE: &'static str = "vault_service";
    pub const IDENTITY_SERVICE: &'static str = "identity_service";
    pub const AUTHENTICATED_SERVICE: &'static str = "authenticated";
    pub const FORWARDING_SERVICE: &'static str = "forwarding_service";
    pub const UPPERCASE_SERVICE: &'static str = "uppercase";
    pub const ECHO_SERVICE: &'static str = "echo";
    pub const HOP_SERVICE: &'static str = "hop";
    pub const CREDENTIALS_SERVICE: &'static str = "credentials";
    pub const SECURE_CHANNEL_LISTENER: &'static str = "api";
    pub const AUTHENTICATOR: &'static str = "authenticator";
    pub const VERIFIER: &'static str = "verifier";
    pub const OKTA_IDENTITY_PROVIDER: &'static str = "okta";
    pub const KAFKA_CONSUMER: &'static str = "kafka_consumer";
    pub const KAFKA_PRODUCER: &'static str = "kafka_producer";
}

pub mod actions {
    use ockam_abac::Action;
    pub const HANDLE_MESSAGE: Action = Action::assert_inline("handle_message");
}

pub mod resources {
    use ockam_abac::Resource;
    pub const INLET: Resource = Resource::assert_inline("tcp-inlet");
    pub const OUTLET: Resource = Resource::assert_inline("tcp-outlet");
}

#[derive(rust_embed::RustEmbed)]
#[folder = "./static"]
pub(crate) struct StaticFiles;
