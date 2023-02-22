use std::borrow::Cow;

use minicbor::{Decode, Encode};
#[cfg(feature = "tag")]
use ockam_core::TypeTag;
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Serialize, Deserialize, Debug)]
#[cfg_attr(test, derive(PartialEq, Eq, Clone))]
#[cbor(transparent)]
#[serde(transparent)]
pub struct Token(#[n(0)] pub String);

impl Token {
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }
}

mod node {
    use minicbor::Decoder;
    use ockam_core::api::Request;
    use ockam_core::{self, AsyncTryClone, Result};
    use ockam_identity::credential::Attributes;
    use ockam_node::Context;
    use tracing::trace;

    use crate::cloud::enroll::auth0::AuthenticateAuth0Token;
    use crate::cloud::enroll::enrollment_token::{EnrollmentToken, RequestEnrollmentToken};
    use crate::cloud::CloudRequestWrapper;
    use crate::nodes::NodeManagerWorker;

    const TARGET: &str = "ockam_api::cloud::enroll";

    impl NodeManagerWorker {
        /// Executes an enrollment process to generate a new set of access tokens using the auth0 flow.
        pub(crate) async fn enroll_auth0(
            &mut self,
            ctx: &mut Context,
            dec: &mut Decoder<'_>,
        ) -> Result<Vec<u8>> {
            let req_wrapper: CloudRequestWrapper<AuthenticateAuth0Token> = dec.decode()?;
            let cloud_route = req_wrapper.route()?;
            let req_body: AuthenticateAuth0Token = req_wrapper.req;
            let req_builder = Request::post("v0/enroll").body(req_body);
            let api_service = "auth0_authenticator";

            trace!(target: TARGET, "executing auth0 flow");

            let ident = {
                let inner = self.get().read().await;
                inner.identity()?.async_try_clone().await?
            };

            self.request_controller(
                ctx,
                api_service,
                None,
                cloud_route,
                api_service,
                req_builder,
                ident,
            )
            .await
        }

        /// Generates a token that will be associated to the passed attributes.
        pub(crate) async fn generate_enrollment_token(
            &mut self,
            ctx: &mut Context,
            dec: &mut Decoder<'_>,
        ) -> Result<Vec<u8>> {
            let req_wrapper: CloudRequestWrapper<Attributes> = dec.decode()?;
            let cloud_route = req_wrapper.route()?;
            let req_body: Attributes = req_wrapper.req;
            let req_body = RequestEnrollmentToken::new(req_body);

            let label = "enrollment_token_generator";
            trace!(target: TARGET, "generating tokens");

            let req_builder = Request::post("v0/").body(req_body);

            let ident = {
                let inner = self.get().read().await;
                inner.identity()?.async_try_clone().await?
            };

            self.request_controller(
                ctx,
                label,
                "request_enrollment_token",
                cloud_route,
                "projects",
                req_builder,
                ident,
            )
            .await
        }

        /// Authenticates a token generated by `generate_enrollment_token`.
        pub(crate) async fn authenticate_enrollment_token(
            &mut self,
            ctx: &mut Context,
            dec: &mut Decoder<'_>,
        ) -> Result<Vec<u8>> {
            let req_wrapper: CloudRequestWrapper<EnrollmentToken> = dec.decode()?;
            let cloud_route = req_wrapper.route()?;
            let req_body: EnrollmentToken = req_wrapper.req;
            let req_builder = Request::post("v0/enroll").body(req_body);
            let api_service = "enrollment_token_authenticator";

            let ident = {
                let inner = self.get().read().await;
                inner.identity()?.async_try_clone().await?
            };

            trace!(target: TARGET, "authenticating token");
            self.request_controller(
                ctx,
                api_service,
                None,
                cloud_route,
                api_service,
                req_builder,
                ident,
            )
            .await
        }
    }
}

pub mod auth0 {
    use super::*;

    // Req/Res types

    #[derive(serde::Deserialize, Debug, PartialEq, Eq)]
    pub struct DeviceCode<'a> {
        pub device_code: Cow<'a, str>,
        pub user_code: Cow<'a, str>,
        pub verification_uri: Cow<'a, str>,
        pub verification_uri_complete: Cow<'a, str>,
        pub expires_in: usize,
        pub interval: usize,
    }

    #[derive(serde::Deserialize, Debug, PartialEq, Eq)]
    pub struct TokensError<'a> {
        pub error: Cow<'a, str>,
        pub error_description: Cow<'a, str>,
    }

    #[derive(serde::Deserialize, Debug)]
    #[cfg_attr(test, derive(PartialEq, Eq, Clone))]
    pub struct Auth0Token {
        pub token_type: TokenType,
        pub access_token: Token,
    }

    #[derive(Encode, Decode, Debug)]
    #[cfg_attr(test, derive(Clone))]
    #[rustfmt::skip]
    #[cbor(map)]
    pub struct AuthenticateAuth0Token {
        #[cfg(feature = "tag")]
        #[n(0)] pub tag: TypeTag<1058055>,
        #[n(1)] pub token_type: TokenType,
        #[n(2)] pub access_token: Token,
    }

    impl AuthenticateAuth0Token {
        pub fn new(token: Auth0Token) -> Self {
            Self {
                #[cfg(feature = "tag")]
                tag: TypeTag,
                token_type: token.token_type,
                access_token: token.access_token,
            }
        }
    }

    // Auxiliary types

    #[derive(serde::Deserialize, Encode, Decode, Debug)]
    #[cfg_attr(test, derive(PartialEq, Eq, Clone))]
    #[rustfmt::skip]
    #[cbor(index_only)]
    pub enum TokenType {
        #[n(0)] Bearer,
    }
}

pub mod enrollment_token {
    use ockam_identity::credential::Attributes;
    use serde::Serialize;

    use super::*;

    // Main req/res types

    #[derive(Encode, Debug)]
    #[cfg_attr(test, derive(Decode, Clone))]
    #[rustfmt::skip]
    #[cbor(map)]
    pub struct RequestEnrollmentToken {
        #[cfg(feature = "tag")]
        #[n(0)] pub tag: TypeTag<8560526>,
        #[b(1)] pub attributes: Attributes,
    }

    impl RequestEnrollmentToken {
        pub fn new(attributes: Attributes) -> Self {
            Self {
                #[cfg(feature = "tag")]
                tag: TypeTag,
                attributes,
            }
        }
    }

    #[derive(Encode, Decode, Serialize, Debug)]
    #[cfg_attr(test, derive(Clone))]
    #[rustfmt::skip]
    #[cbor(map)]
    pub struct EnrollmentToken {
        #[cfg(feature = "tag")]
        #[serde(skip)]
        #[n(0)] pub tag: TypeTag<8932763>,
        #[n(1)] pub token: Token,
    }

    impl EnrollmentToken {
        pub fn new(token: Token) -> Self {
            Self {
                #[cfg(feature = "tag")]
                tag: TypeTag,
                token,
            }
        }
    }

    #[derive(Encode, Debug)]
    #[cfg_attr(test, derive(Decode, Clone))]
    #[rustfmt::skip]
    #[cbor(map)]
    pub struct AuthenticateEnrollmentToken {
        #[cfg(feature = "tag")]
        #[n(0)] pub tag: TypeTag<9463780>,
        #[n(1)] pub token: Token,
    }

    impl AuthenticateEnrollmentToken {
        pub fn new(token: EnrollmentToken) -> Self {
            Self {
                #[cfg(feature = "tag")]
                tag: TypeTag,
                token: token.token,
            }
        }
    }
}
