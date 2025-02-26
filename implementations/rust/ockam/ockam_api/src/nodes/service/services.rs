use minicbor::Decoder;
use ockam::{Address, AsyncTryClone, Context, Result};
use ockam_core::api::{Request, Response, ResponseBuilder};
use ockam_core::{AllowAll, Route};
use ockam_multiaddr::MultiAddr;

use super::NodeManagerWorker;
use crate::auth::Server;
use crate::echoer::Echoer;
use crate::error::ApiError;
use crate::hop::Hop;
use crate::identity::IdentityService;
use crate::kafka::{KafkaPortalListener, KAFKA_BOOTSTRAP_ADDRESS, KAFKA_INTERCEPTOR_ADDRESS};
use crate::nodes::models::services::{
    ServiceList,
    ServiceStatus,
    StartAuthenticatedServiceRequest,
    StartAuthenticatorRequest,
    StartCredentialsService,
    StartEchoerServiceRequest,
    StartHopServiceRequest,
    StartIdentityServiceRequest,
    StartKafkaConsumerRequest,
    StartKafkaProducerRequest,
    StartOktaIdentityProviderRequest,
    StartServiceRequest,
    StartUppercaseServiceRequest,
    StartVaultServiceRequest,
    StartVerifierService,
};
use crate::nodes::registry::{
    CredentialsServiceInfo,
    KafkaServiceInfo,
    KafkaServiceKind,
    Registry,
    VerifierServiceInfo,
};
use crate::nodes::NodeManager;
use crate::port_range::PortRange;
use crate::uppercase::Uppercase;
use crate::vault::VaultService;
use crate::{try_multiaddr_to_route, DefaultAddress};

impl NodeManager {
    pub(super) async fn start_vault_service_impl(
        &mut self,
        ctx: &Context,
        addr: Address,
    ) -> Result<()> {
        if self.registry.vault_services.contains_key(&addr) {
            return Err(ApiError::generic("Vault service exists at this address"));
        }

        let vault = self.vault()?.async_try_clone().await?;
        let service = VaultService::new(vault);

        ctx.start_worker(
            addr.clone(),
            service,
            AllowAll, // FIXME: @ac
            AllowAll,
        )
        .await?;

        self.registry
            .vault_services
            .insert(addr, Default::default());

        Ok(())
    }

    pub(super) async fn start_identity_service_impl(
        &mut self,
        ctx: &Context,
        addr: Address,
    ) -> Result<()> {
        if self.registry.identity_services.contains_key(&addr) {
            return Err(ApiError::generic("Identity service exists at this address"));
        }

        let vault = self.vault()?.async_try_clone().await?;
        let service = IdentityService::new(ctx, vault).await?;

        ctx.start_worker(
            addr.clone(),
            service,
            AllowAll, // FIXME: @ac
            AllowAll,
        )
        .await?;

        self.registry
            .identity_services
            .insert(addr, Default::default());

        Ok(())
    }

    pub(super) async fn start_credentials_service_impl<'a>(
        &mut self,
        addr: Address,
        oneway: bool,
    ) -> Result<()> {
        if self.registry.credentials_services.contains_key(&addr) {
            return Err(ApiError::generic(
                "Credentials service exists at this address",
            ));
        }

        let identity = self.identity()?;

        let authorities = self.authorities()?;

        identity
            .start_credential_exchange_worker(
                authorities.public_identities(),
                addr.clone(),
                !oneway,
                self.attributes_storage.async_try_clone().await?,
            )
            .await?;

        self.registry
            .credentials_services
            .insert(addr, CredentialsServiceInfo::default());

        Ok(())
    }

    pub(super) async fn start_authenticated_service_impl(
        &mut self,
        ctx: &Context,
        addr: Address,
    ) -> Result<()> {
        if self.registry.authenticated_services.contains_key(&addr) {
            return Err(ApiError::generic(
                "Authenticated service exists at this address",
            ));
        }

        let s = self.attributes_storage.async_try_clone().await?;
        let server = Server::new(s);
        ctx.start_worker(
            addr.clone(),
            server,
            AllowAll, // FIXME: @ac
            AllowAll,
        )
        .await?;

        self.registry
            .authenticated_services
            .insert(addr, Default::default());

        Ok(())
    }

    pub(super) async fn start_uppercase_service_impl(
        &mut self,
        ctx: &Context,
        addr: Address,
    ) -> Result<()> {
        if self.registry.uppercase_services.contains_key(&addr) {
            return Err(ApiError::generic(
                "Uppercase service exists at this address",
            ));
        }

        ctx.start_worker(
            addr.clone(),
            Uppercase,
            AllowAll, // FIXME: @ac
            AllowAll,
        )
        .await?;

        self.registry
            .uppercase_services
            .insert(addr, Default::default());

        Ok(())
    }

    pub(super) async fn start_echoer_service_impl(
        &mut self,
        ctx: &Context,
        addr: Address,
    ) -> Result<()> {
        if self.registry.echoer_services.contains_key(&addr) {
            return Err(ApiError::generic("Echoer service exists at this address"));
        }

        ctx.start_worker(
            addr.clone(),
            Echoer,
            AllowAll, // FIXME: @ac
            AllowAll,
        )
        .await?;

        self.registry
            .echoer_services
            .insert(addr, Default::default());

        Ok(())
    }

    pub(super) async fn start_hop_service_impl(
        &mut self,
        ctx: &Context,
        addr: Address,
    ) -> Result<()> {
        if self.registry.hop_services.contains_key(&addr) {
            return Err(ApiError::generic("Hop service exists at this address"));
        }

        ctx.start_worker(
            addr.clone(),
            Hop,
            AllowAll, // FIXME: @ac
            AllowAll,
        )
        .await?;

        self.registry.hop_services.insert(addr, Default::default());

        Ok(())
    }

    #[cfg(feature = "direct-authenticator")]
    pub(super) async fn start_direct_authenticator_service_impl(
        &mut self,
        ctx: &Context,
        addr: Address,
        enrollers: &str,
        reload_enrollers: bool,
        proj: &[u8],
    ) -> Result<()> {
        use crate::nodes::registry::AuthenticatorServiceInfo;
        if self.registry.authenticator_service.contains_key(&addr) {
            return Err(ApiError::generic("Authenticator service already started"));
        }
        let db = self.attributes_storage.async_try_clone().await?;
        let id = self.identity()?.async_try_clone().await?;
        let au = crate::authenticator::direct::Server::new(
            proj.to_vec(),
            db,
            enrollers,
            reload_enrollers,
            id,
        )
        .await?;
        ctx.start_worker(
            addr.clone(),
            au,
            AllowAll, // FIXME: @ac
            AllowAll,
        )
        .await?;
        self.registry
            .authenticator_service
            .insert(addr, AuthenticatorServiceInfo::default());
        Ok(())
    }

    pub(super) async fn start_okta_identity_provider_service_impl(
        &mut self,
        ctx: &Context,
        addr: Address,
        tenant_base_url: &str,
        certificate: &str,
        attributes: &[&str],
        proj: &[u8],
    ) -> Result<()> {
        use crate::nodes::registry::OktaIdentityProviderServiceInfo;
        if self
            .registry
            .okta_identity_provider_services
            .contains_key(&addr)
        {
            return Err(ApiError::generic(
                "Okta Identity Provider service already started",
            ));
        }
        let db = self.authenticated_storage.async_try_clone().await?;
        let au =
            crate::okta::Server::new(proj.to_vec(), db, tenant_base_url, certificate, attributes)?;
        ctx.start_worker(
            addr.clone(),
            au,
            AllowAll, // FIXME: @ac
            AllowAll,
        )
        .await?;
        self.registry
            .okta_identity_provider_services
            .insert(addr, OktaIdentityProviderServiceInfo::default());
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn start_kafka_service_impl<'a>(
        &mut self,
        context: &Context,
        listener_address: Address,
        bind_ip: String,
        proxied_bootstrap_port: u16,
        proxied_port_range: (u16, u16),
        forwarding_addr: MultiAddr,
        kind: KafkaServiceKind,
    ) -> Result<()> {
        let node_route = try_multiaddr_to_route(&forwarding_addr)?;
        // We manipulate the route a bit, adding common pieces for both
        // bootstrap route and broker route
        let interceptor_route: Route = node_route
            .clone()
            .modify()
            .prepend(listener_address.clone())
            .append(Address::from_string(KAFKA_INTERCEPTOR_ADDRESS))
            .into();

        self.tcp_transport
            .create_inlet(
                format!("{}:{}", &bind_ip, proxied_bootstrap_port),
                interceptor_route
                    .clone()
                    .modify()
                    .append(Address::from_string(KAFKA_BOOTSTRAP_ADDRESS)),
                AllowAll,
            )
            .await?;

        KafkaPortalListener::start(
            context,
            interceptor_route,
            listener_address.clone(),
            bind_ip,
            PortRange::try_from(proxied_port_range)
                .map_err(|_| ApiError::message("invalid port range"))?,
        )
        .await?;

        self.registry
            .kafka_services
            .insert(listener_address, KafkaServiceInfo::new(kind));
        Ok(())
    }
}

impl NodeManagerWorker {
    pub(super) async fn start_vault_service(
        &mut self,
        ctx: &Context,
        req: &Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<ResponseBuilder> {
        let mut node_manager = self.node_manager.write().await;
        let req_body: StartVaultServiceRequest = dec.decode()?;
        let addr = req_body.addr.to_string().into();
        node_manager.start_vault_service_impl(ctx, addr).await?;
        Ok(Response::ok(req.id()))
    }

    pub(super) async fn start_identity_service(
        &mut self,
        ctx: &Context,
        req: &Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<ResponseBuilder> {
        let mut node_manager = self.node_manager.write().await;
        let req_body: StartIdentityServiceRequest = dec.decode()?;
        let addr = req_body.addr.to_string().into();
        node_manager.start_identity_service_impl(ctx, addr).await?;
        Ok(Response::ok(req.id()))
    }

    pub(super) async fn start_authenticated_service(
        &mut self,
        ctx: &Context,
        req: &Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<ResponseBuilder> {
        let mut node_manager = self.node_manager.write().await;
        let req_body: StartAuthenticatedServiceRequest = dec.decode()?;
        let addr = req_body.addr.to_string().into();
        node_manager
            .start_authenticated_service_impl(ctx, addr)
            .await?;
        Ok(Response::ok(req.id()))
    }

    pub(super) async fn start_uppercase_service(
        &mut self,
        ctx: &Context,
        req: &Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<ResponseBuilder> {
        let mut node_manager = self.node_manager.write().await;
        let req_body: StartUppercaseServiceRequest = dec.decode()?;
        let addr = req_body.addr.to_string().into();
        node_manager.start_uppercase_service_impl(ctx, addr).await?;
        Ok(Response::ok(req.id()))
    }

    pub(super) async fn start_echoer_service(
        &mut self,
        ctx: &Context,
        req: &Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<ResponseBuilder> {
        let mut node_manager = self.node_manager.write().await;
        let req_body: StartEchoerServiceRequest = dec.decode()?;
        let addr = req_body.addr.to_string().into();
        node_manager.start_echoer_service_impl(ctx, addr).await?;
        Ok(Response::ok(req.id()))
    }

    pub(super) async fn start_hop_service(
        &mut self,
        ctx: &Context,
        req: &Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<ResponseBuilder> {
        let mut node_manager = self.node_manager.write().await;
        let req_body: StartHopServiceRequest = dec.decode()?;
        let addr = req_body.addr.to_string().into();
        node_manager.start_hop_service_impl(ctx, addr).await?;
        Ok(Response::ok(req.id()))
    }

    pub(super) async fn start_authenticator_service<'a>(
        &mut self,
        ctx: &Context,
        req: &'a Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<ResponseBuilder> {
        let mut node_manager = self.node_manager.write().await;
        #[cfg(not(feature = "direct-authenticator"))]
        return Err(ApiError::generic("Direct authenticator not available"));

        #[cfg(feature = "direct-authenticator")]
        {
            let body: StartAuthenticatorRequest = dec.decode()?;
            let addr: Address = body.address().into();

            node_manager
                .start_direct_authenticator_service_impl(
                    ctx,
                    addr,
                    body.enrollers(),
                    body.reload_enrollers(),
                    body.project(),
                )
                .await?;
        }

        Ok(Response::ok(req.id()))
    }

    pub(super) async fn start_okta_identity_provider_service<'a>(
        &mut self,
        ctx: &Context,
        req: &'a Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<ResponseBuilder> {
        let mut node_manager = self.node_manager.write().await;
        let body: StartOktaIdentityProviderRequest = dec.decode()?;
        let addr: Address = body.address().into();
        node_manager
            .start_okta_identity_provider_service_impl(
                ctx,
                addr,
                body.tenant_base_url(),
                body.certificate(),
                body.attributes(),
                body.project(),
            )
            .await?;
        Ok(Response::ok(req.id()))
    }

    pub(super) async fn start_verifier_service<'a>(
        &mut self,
        ctx: &Context,
        req: &'a Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<ResponseBuilder> {
        let mut node_manager = self.node_manager.write().await;
        let body: StartVerifierService = dec.decode()?;
        let addr: Address = body.address().into();

        if node_manager.registry.verifier_services.contains_key(&addr) {
            return Err(ApiError::generic("Verifier service exists at this address"));
        }

        let vault = node_manager.vault.async_try_clone().await?;
        let vs = crate::verifier::Verifier::new(vault);
        ctx.start_worker(
            addr.clone(),
            vs,
            AllowAll, // FIXME: @ac
            AllowAll,
        )
        .await?;

        node_manager
            .registry
            .verifier_services
            .insert(addr, VerifierServiceInfo::default());

        Ok(Response::ok(req.id()))
    }

    pub(super) async fn start_credentials_service<'a>(
        &mut self,
        _ctx: &Context,
        req: &'a Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<ResponseBuilder> {
        let mut node_manager = self.node_manager.write().await;
        let body: StartCredentialsService = dec.decode()?;
        let addr: Address = body.address().into();
        let oneway = body.oneway();

        node_manager
            .start_credentials_service_impl(addr, oneway)
            .await?;

        Ok(Response::ok(req.id()))
    }

    pub(super) async fn start_kafka_consumer_service<'a>(
        &mut self,
        context: &Context,
        req: &'a Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<Vec<u8>> {
        let mut node_manager = self.node_manager.write().await;
        let body: StartServiceRequest<StartKafkaConsumerRequest> = dec.decode()?;
        let listener_address: Address = body.address().into();
        let body_req = body.request();

        node_manager
            .start_kafka_service_impl(
                context,
                listener_address,
                body_req.ip().to_string(),
                body_req.bootstrap_port(),
                body_req.port_range(),
                body_req.forwarding_addr().to_string().parse()?,
                KafkaServiceKind::Consumer,
            )
            .await?;

        Ok(Response::ok(req.id()).to_vec()?)
    }

    pub(super) async fn start_kafka_producer_service<'a>(
        &mut self,
        context: &Context,
        req: &'a Request<'_>,
        dec: &mut Decoder<'_>,
    ) -> Result<Vec<u8>> {
        let mut node_manager = self.node_manager.write().await;
        let body: StartServiceRequest<StartKafkaProducerRequest> = dec.decode()?;
        let listener_address: Address = body.address().into();
        let body_req = body.request();

        node_manager
            .start_kafka_service_impl(
                context,
                listener_address,
                body_req.ip().to_string(),
                body_req.bootstrap_port(),
                body_req.port_range(),
                body_req.forwarding_addr().to_string().parse()?,
                KafkaServiceKind::Producer,
            )
            .await?;

        Ok(Response::ok(req.id()).to_vec()?)
    }

    pub(super) fn list_services<'a>(
        &self,
        req: &Request<'a>,
        registry: &'a Registry,
    ) -> ResponseBuilder<ServiceList<'a>> {
        let mut list = Vec::new();
        registry.vault_services.keys().for_each(|addr| {
            list.push(ServiceStatus::new(
                addr.address(),
                DefaultAddress::VAULT_SERVICE,
            ))
        });
        registry.identity_services.keys().for_each(|addr| {
            list.push(ServiceStatus::new(
                addr.address(),
                DefaultAddress::IDENTITY_SERVICE,
            ))
        });
        registry.authenticated_services.keys().for_each(|addr| {
            list.push(ServiceStatus::new(
                addr.address(),
                DefaultAddress::AUTHENTICATED_SERVICE,
            ))
        });
        registry.uppercase_services.keys().for_each(|addr| {
            list.push(ServiceStatus::new(
                addr.address(),
                DefaultAddress::UPPERCASE_SERVICE,
            ))
        });
        registry.echoer_services.keys().for_each(|addr| {
            list.push(ServiceStatus::new(
                addr.address(),
                DefaultAddress::ECHO_SERVICE,
            ))
        });
        registry.hop_services.keys().for_each(|addr| {
            list.push(ServiceStatus::new(
                addr.address(),
                DefaultAddress::HOP_SERVICE,
            ))
        });
        registry.verifier_services.keys().for_each(|addr| {
            list.push(ServiceStatus::new(addr.address(), DefaultAddress::VERIFIER))
        });
        registry.credentials_services.keys().for_each(|addr| {
            list.push(ServiceStatus::new(
                addr.address(),
                DefaultAddress::CREDENTIALS_SERVICE,
            ))
        });
        registry.kafka_services.iter().for_each(|(address, info)| {
            list.push(ServiceStatus::new(
                address.address(),
                match info.kind() {
                    KafkaServiceKind::Consumer => "kafka-consumer",
                    KafkaServiceKind::Producer => "kafka-producer",
                },
            ))
        });

        #[cfg(feature = "direct-authenticator")]
        registry.authenticator_service.keys().for_each(|addr| {
            list.push(ServiceStatus::new(
                addr.address(),
                DefaultAddress::AUTHENTICATOR,
            ))
        });

        Response::ok(req.id()).body(ServiceList::new(list))
    }
}
