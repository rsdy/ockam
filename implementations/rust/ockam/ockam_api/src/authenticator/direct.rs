pub mod types;

use core::{fmt, str};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::Path;
use std::time::{Duration, Instant};

use lru::LruCache;
use minicbor::{Decoder, Encode};
use ockam::identity::authenticated_storage::{
    AttributesEntry,
    AuthenticatedStorage,
    IdentityAttributeStorage,
};
use ockam::identity::credential::{Credential, OneTimeCode, SchemaId, Timestamp};
use ockam::identity::{
    Identity,
    IdentityIdentifier,
    IdentitySecureChannelLocalInfo,
    IdentityVault,
};
use ockam_core::api::{
    self,
    assert_request_match,
    assert_response_match,
    Error,
    Method,
    Request,
    RequestBuilder,
    Response,
    ResponseBuilder,
    Status,
};
use ockam_core::errcode::{Kind, Origin};
use ockam_core::{self, Address, DenyAll, Result, Route, Routed, Worker};
use ockam_node::Context;
use serde_json as json;
use tracing::{trace, warn};
use types::AddMember;

use self::types::Enroller;
use crate::authenticator::direct::types::CreateToken;

const LEGACY_MEMBER: &str = "member";
const MAX_TOKEN_DURATION: Duration = Duration::from_secs(600);

/// Schema identifier for a project membership credential.
///
/// The credential will consist of the following attributes:
///
/// - `project_id` : bytes
/// - `role`: b"member"
pub const PROJECT_MEMBER_SCHEMA: SchemaId = SchemaId(1);
pub const PROJECT_ID: &str = "project_id";

pub struct Server<S: AuthenticatedStorage, IS: IdentityAttributeStorage, V: IdentityVault> {
    project: Vec<u8>,
    store: IS,
    ident: Identity<V, S>,
    filename: Option<String>,
    enrollers: HashMap<IdentityIdentifier, Enroller>,
    reload_enrollers: bool,
    tokens: LruCache<[u8; 32], Token>,
}

struct Token {
    attrs: HashMap<String, String>,
    generated_by: IdentityIdentifier,
    time: Instant,
}

#[ockam_core::worker]
impl<S, IS, V> Worker for Server<S, IS, V>
where
    S: AuthenticatedStorage,
    IS: IdentityAttributeStorage,
    V: IdentityVault,
{
    type Context = Context;
    type Message = Vec<u8>;

    async fn handle_message(&mut self, c: &mut Context, m: Routed<Self::Message>) -> Result<()> {
        if let Ok(i) = IdentitySecureChannelLocalInfo::find_info(m.local_message()) {
            let r = self.on_request(i.their_identity_id(), m.as_body()).await?;
            c.send(m.return_route(), r).await
        } else {
            let mut dec = Decoder::new(m.as_body());
            let req: Request = dec.decode()?;
            let res = api::forbidden(&req, "secure channel required").to_vec()?;
            c.send(m.return_route(), res).await
        }
    }
}

impl<S, IS, V> Server<S, IS, V>
where
    S: AuthenticatedStorage,
    IS: IdentityAttributeStorage,
    V: IdentityVault,
{
    pub async fn new(
        project: Vec<u8>,
        store: IS,
        enrollers: &str,
        reload_enrollers: bool,
        identity: Identity<V, S>,
    ) -> Result<Self> {
        let (filename, enrollers_data) = Self::parse_enrollers(enrollers)?;

        //TODO: This block is from converting old-style member' data into
        //      the new format suitable for our ABAC framework.  Remove it
        //      once we don't have more legacy data around.
        let legacy_s = identity.authenticated_storage();
        for k in legacy_s.keys(LEGACY_MEMBER).await? {
            if let Some(data) = legacy_s.get(&k, LEGACY_MEMBER).await? {
                let m: HashMap<String, String> = minicbor::decode(&data)?;
                let attrs = m
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.as_bytes().to_vec()))
                    .collect();
                let entry = AttributesEntry::new(attrs, Timestamp::now().unwrap(), None, None);
                let identifier = IdentityIdentifier::try_from(k.clone())?;
                store.put_attributes(&identifier, entry).await?;
            }
            legacy_s.del(&k, LEGACY_MEMBER).await?;
        }

        Ok(Server {
            project,
            store,
            ident: identity,
            filename,
            enrollers: enrollers_data,
            reload_enrollers,
            tokens: LruCache::new(NonZeroUsize::new(128).expect("0 < 128")),
        })
    }

    fn parse_enrollers(
        json_or_path: &str,
    ) -> Result<(Option<String>, HashMap<IdentityIdentifier, Enroller>)> {
        match json::from_str::<HashMap<IdentityIdentifier, Enroller>>(json_or_path) {
            Ok(enrollers) => Ok((None, enrollers)),
            Err(_) => {
                let contents = std::fs::read_to_string(json_or_path)
                    .map_err(|e| ockam_core::Error::new(Origin::Other, Kind::Io, e))?;

                let enrollers = json::from_str(&contents)
                    .map_err(|e| ockam_core::Error::new(Origin::Other, Kind::Invalid, e))?;

                Ok((Some(json_or_path.to_string()), enrollers))
            }
        }
    }

    async fn on_request(&mut self, from: &IdentityIdentifier, data: &[u8]) -> Result<Vec<u8>> {
        let mut dec = Decoder::new(data);
        let req: Request = dec.decode()?;

        trace! {
            target: "ockam_api::authenticator::direct::server",
            from   = %from,
            id     = %req.id(),
            method = ?req.method(),
            path   = %req.path(),
            body   = %req.has_body(),
            "request"
        }

        let res = match req.method() {
            Some(Method::Post) => match req.path_segments::<2>().as_slice() {
                // Enroller wants to create an enrollment token.
                ["tokens"] => match self.check_enroller(&req, from).await {
                    Ok(None) => {
                        let att: CreateToken = dec.decode()?;
                        let otc = OneTimeCode::new();
                        let res = Response::ok(req.id()).body(&otc).to_vec()?;
                        let tkn = Token {
                            attrs: att.into_owned_attributes(),
                            generated_by: from.clone(),
                            time: Instant::now(),
                        };
                        self.tokens.put(*otc.code(), tkn);
                        res
                    }
                    Ok(Some(e)) => e.to_vec()?,
                    Err(e) => api::internal_error(&req, &e.to_string()).to_vec()?,
                },
                // Enroller wants to add a member.
                ["members"] => match self.check_enroller(&req, from).await {
                    Ok(None) => {
                        let add: AddMember = dec.decode()?;
                        //TODO: fixme:  unify use of hashmap vs btreemap
                        let attrs = add
                            .attributes()
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.as_bytes().to_vec()))
                            .collect();
                        let entry = AttributesEntry::new(
                            attrs,
                            Timestamp::now().unwrap(),
                            None,
                            Some(from.clone()),
                        );
                        self.store.put_attributes(add.member(), entry).await?;
                        Response::ok(req.id()).to_vec()?
                    }
                    Ok(Some(e)) => e.to_vec()?,
                    Err(error) => api::internal_error(&req, &error.to_string()).to_vec()?,
                },
                // New member with an enrollment token wants its first credential.
                ["credential"] if req.has_body() => {
                    let otc: OneTimeCode = dec.decode()?;
                    if let Some(tkn) = self.tokens.pop(otc.code()) {
                        if tkn.time.elapsed() > MAX_TOKEN_DURATION {
                            api::forbidden(&req, "expired token").to_vec()?
                        } else {
                            //TODO: fixme:  unify use of hashmap vs btreemap
                            let attrs = tkn
                                .attrs
                                .iter()
                                .map(|(k, v)| (k.to_string(), v.as_bytes().to_vec()))
                                .collect();
                            let entry = AttributesEntry::new(
                                attrs,
                                Timestamp::now().unwrap(),
                                None,
                                Some(tkn.generated_by),
                            );
                            self.store.put_attributes(from, entry).await?;
                            //TODO: use the entry not the token
                            let crd = tkn
                                .attrs
                                .iter()
                                .fold(Credential::builder(from.clone()), |crd, (a, v)| {
                                    crd.with_attribute(a, v.as_bytes())
                                })
                                .with_schema(PROJECT_MEMBER_SCHEMA)
                                .with_attribute(PROJECT_ID, &self.project);
                            let crd = self.ident.issue_credential(crd).await?;
                            Response::ok(req.id()).body(crd).to_vec()?
                        }
                    } else {
                        api::forbidden(&req, "unknown token").to_vec()?
                    }
                }
                // Member wants a credential.
                ["credential"] => match self.store.get_attributes(from).await {
                    Ok(Some(entry)) => {
                        let crd = entry
                            .attrs()
                            .iter()
                            .fold(
                                Credential::builder(from.clone())
                                    .with_schema(PROJECT_MEMBER_SCHEMA),
                                |crd, (a, v)| crd.with_attribute(a, v),
                            )
                            .with_attribute(PROJECT_ID, &self.project);
                        let crd = self.ident.issue_credential(crd).await?;
                        Response::ok(req.id()).body(crd).to_vec()?
                    }
                    Ok(None) => api::forbidden(&req, "unauthorized member").to_vec()?,
                    Err(error) => api::internal_error(&req, &error.to_string()).to_vec()?,
                },
                _ => api::unknown_path(&req).to_vec()?,
            },
            _ => api::invalid_method(&req).to_vec()?,
        };

        Ok(res)
    }

    async fn check_enroller<'a>(
        &mut self,
        req: &'a Request<'_>,
        enroller: &IdentityIdentifier,
    ) -> Result<Option<ResponseBuilder<Error<'a>>>> {
        if self.reload_enrollers && self.filename.is_some() {
            let filename = self.filename.as_ref().unwrap();
            let path = Path::new(&filename);
            let contents = std::fs::read_to_string(path)
                .map_err(|e| ockam_core::Error::new(Origin::Other, Kind::Io, e))?;

            let enrollers: HashMap<IdentityIdentifier, Enroller> = json::from_str(&contents)
                .map_err(|e| ockam_core::Error::new(Origin::Other, Kind::Invalid, e))?;

            self.enrollers = enrollers;
        }

        if self.enrollers.contains_key(enroller) {
            return Ok(None);
        }

        warn! {
            target: "ockam_api::authenticator::direct::server",
            enroller = %enroller,
            id       = %req.id(),
            method   = ?req.method(),
            path     = %req.path(),
            body     = %req.has_body(),
            "unauthorised enroller"
        }

        Ok(Some(api::forbidden(req, "unauthorized enroller")))
    }
}

pub struct Client {
    ctx: Context,
    route: Route,
    buf: Vec<u8>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("route", &self.route)
            .finish()
    }
}

impl Client {
    pub async fn new(r: Route, ctx: &Context) -> Result<Self> {
        let ctx = ctx
            .new_detached(
                Address::random_tagged("AuthClient.direct.detached"),
                DenyAll,
                DenyAll,
            )
            .await?;
        Ok(Client {
            ctx,
            route: r,
            buf: Vec::new(),
        })
    }

    pub async fn add_member(
        &mut self,
        id: IdentityIdentifier,
        attributes: HashMap<&str, &str>,
    ) -> Result<()> {
        let req = Request::post("/members").body(AddMember::new(id).with_attributes(attributes));
        self.buf = self.request("add-member", "add_member", &req).await?;
        assert_response_match(None, &self.buf);
        let mut d = Decoder::new(&self.buf);
        let res = response("add-member", &mut d)?;
        if res.status() == Some(Status::Ok) {
            Ok(())
        } else {
            Err(error("add-member", &res, &mut d))
        }
    }

    pub async fn create_token(&mut self, attributes: HashMap<&str, &str>) -> Result<OneTimeCode> {
        let req = Request::post("/tokens").body(CreateToken::new().with_attributes(attributes));
        self.buf = self.request("create-token", "create_token", &req).await?;
        assert_response_match("onetime_code", &self.buf);
        let mut d = Decoder::new(&self.buf);
        let res = response("create-token", &mut d)?;
        if res.status() == Some(Status::Ok) {
            Ok(d.decode()?)
        } else {
            Err(error("create-token", &res, &mut d))
        }
    }

    pub async fn credential(&mut self) -> Result<Credential> {
        let req = Request::post("/credential");
        self.buf = self.request("new-credential", None, &req).await?;
        assert_response_match("credential", &self.buf);
        let mut d = Decoder::new(&self.buf);
        let res = response("new-credential", &mut d)?;
        if res.status() == Some(Status::Ok) {
            Ok(d.decode()?)
        } else {
            Err(error("new-credential", &res, &mut d))
        }
    }

    pub async fn credential_with(&mut self, c: &OneTimeCode) -> Result<Credential> {
        let req = Request::post("/credential").body(c);
        self.buf = self.request("new-credential", None, &req).await?;
        assert_response_match("credential", &self.buf);
        let mut d = Decoder::new(&self.buf);
        let res = response("new-credential", &mut d)?;
        if res.status() == Some(Status::Ok) {
            Ok(d.decode()?)
        } else {
            Err(error("new-credential", &res, &mut d))
        }
    }

    /// Encode request header and body (if any) and send the package to the server.
    async fn request<T>(
        &mut self,
        label: &str,
        schema: impl Into<Option<&str>>,
        req: &RequestBuilder<'_, T>,
    ) -> Result<Vec<u8>>
    where
        T: Encode<()>,
    {
        let mut buf = Vec::new();
        req.encode(&mut buf)?;
        assert_request_match(schema, &buf);
        trace! {
            target: "ockam_api::authenticator::direct::client",
            id     = %req.header().id(),
            method = ?req.header().method(),
            path   = %req.header().path(),
            body   = %req.header().has_body(),
            "-> {label}"
        };
        let vec: Vec<u8> = self.ctx.send_and_receive(self.route.clone(), buf).await?;
        Ok(vec)
    }
}

/// Decode and log response header.
fn response(label: &str, dec: &mut Decoder<'_>) -> Result<Response> {
    let res: Response = dec.decode()?;
    trace! {
        target: "ockam_api::authenticator::direct::client",
        re     = %res.re(),
        id     = %res.id(),
        status = ?res.status(),
        body   = %res.has_body(),
        "<- {label}"
    }
    Ok(res)
}

/// Decode, log and map response error to ockam_core error.
fn error(label: &str, res: &Response, dec: &mut Decoder<'_>) -> ockam_core::Error {
    if res.has_body() {
        let err = match dec.decode::<Error>() {
            Ok(e) => e,
            Err(e) => return e.into(),
        };
        warn! {
            target: "ockam_api::authenticator::direct::client",
            id     = %res.id(),
            re     = %res.re(),
            status = ?res.status(),
            error  = ?err.message(),
            "<- {label}"
        }
        let msg = err.message().unwrap_or(label);
        ockam_core::Error::new(Origin::Application, Kind::Protocol, msg)
    } else {
        ockam_core::Error::new(Origin::Application, Kind::Protocol, label)
    }
}
