use ockam::identity::IdentityIdentifier;
use ockam_api::cloud::project::{OktaConfig, Project};
use ockam_core::CowStr;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProjectInfo<'a> {
    #[serde(borrow)]
    pub id: CowStr<'a>,
    #[serde(borrow)]
    pub name: CowStr<'a>,
    pub identity: Option<IdentityIdentifier>,
    #[serde(borrow)]
    pub access_route: CowStr<'a>,
    #[serde(borrow)]
    pub authority_access_route: Option<CowStr<'a>>,
    #[serde(borrow)]
    pub authority_identity: Option<CowStr<'a>>,
    #[serde(borrow)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub okta_config: Option<OktaConfig<'a>>,
}

impl<'a> From<Project<'a>> for ProjectInfo<'a> {
    fn from(p: Project<'a>) -> Self {
        Self {
            id: p.id,
            name: p.name,
            identity: p.identity,
            access_route: p.access_route,
            authority_access_route: p.authority_access_route,
            authority_identity: p.authority_identity,
            okta_config: p.okta_config,
        }
    }
}

impl<'a> From<&ProjectInfo<'a>> for Project<'a> {
    fn from(p: &ProjectInfo<'a>) -> Self {
        Project {
            id: p.id.clone(),
            name: p.name.clone(),
            identity: p.identity.clone(),
            access_route: p.access_route.clone(),
            authority_access_route: p.authority_access_route.clone(),
            authority_identity: p.authority_identity.clone(),
            okta_config: p.okta_config.clone(),
            ..Default::default()
        }
    }
}
