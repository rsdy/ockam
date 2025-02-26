#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate core;

#[cfg(feature = "alloc")]
extern crate alloc;

use arrayref::array_ref;
use ockam_core::vault::{
    AsymmetricVault, Hasher, PublicKey, SecretType, SecretVault, Signer, SymmetricVault, Verifier,
};
use ockam_core::{compat::vec::Vec, AsyncTryClone};
use zeroize::Zeroize;

mod error;
pub use error::*;

mod initiator;
pub use initiator::*;
mod responder;
pub use responder::*;
mod new_key_exchanger;
pub use new_key_exchanger::*;

/// Represents and (X)EdDSA or ECDSA signature
/// from Ed25519 or P-256
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct Signature([u8; 64]);

impl AsRef<[u8; 64]> for Signature {
    fn as_ref(&self) -> &[u8; 64] {
        &self.0
    }
}

impl From<[u8; 64]> for Signature {
    fn from(data: [u8; 64]) -> Self {
        Signature(data)
    }
}

impl From<&[u8; 64]> for Signature {
    fn from(data: &[u8; 64]) -> Self {
        Signature(*data)
    }
}

impl core::fmt::Debug for Signature {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Signature {{ {} }}", hex::encode(self.0.as_ref()))
    }
}

/// Represents all the keys and signature to send to an enrollee
#[derive(Clone, Debug, Zeroize)]
#[zeroize(drop)]
pub struct PreKeyBundle {
    identity_key: PublicKey,
    signed_prekey: PublicKey,
    signature_prekey: Signature,
    one_time_prekey: PublicKey,
}

impl PreKeyBundle {
    const SIZE: usize = 32 + 32 + 64 + 32;
    /// Convert the prekey bundle to a byte array
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(self.identity_key.data());
        output.extend_from_slice(self.signed_prekey.data());
        output.extend_from_slice(self.signature_prekey.0.as_ref());
        output.extend_from_slice(self.one_time_prekey.data());
        output
    }
}

impl TryFrom<&[u8]> for PreKeyBundle {
    type Error = ockam_core::Error;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != Self::SIZE {
            return Err(X3DHError::MessageLenMismatch.into());
        }
        let identity_key = PublicKey::new(array_ref![data, 0, 32].to_vec(), SecretType::X25519);
        let signed_prekey = PublicKey::new(array_ref![data, 32, 32].to_vec(), SecretType::X25519);
        let signature_prekey = Signature(*array_ref![data, 64, 64]);
        let one_time_prekey =
            PublicKey::new(array_ref![data, 128, 32].to_vec(), SecretType::X25519);
        Ok(Self {
            identity_key,
            signed_prekey,
            signature_prekey,
            one_time_prekey,
        })
    }
}

const CSUITE: &[u8] = b"X3DH_25519_AESGCM_SHA256\0\0\0\0\0\0\0\0";

/// Vault with X3DH required functionality
pub trait X3dhVault:
    SecretVault
    + Signer
    + Verifier
    + AsymmetricVault
    + SymmetricVault
    + Hasher
    + AsyncTryClone
    + Send
    + Sync
    + 'static
{
}

impl<D> X3dhVault for D where
    D: SecretVault
        + Signer
        + Verifier
        + AsymmetricVault
        + SymmetricVault
        + Hasher
        + AsyncTryClone
        + Send
        + Sync
        + 'static
{
}

#[cfg(test)]
mod tests {
    use super::*;
    use ockam_core::Result;
    use ockam_key_exchange_core::{KeyExchanger, NewKeyExchanger};
    use ockam_node::Context;
    use ockam_vault::Vault;

    #[allow(non_snake_case)]
    #[ockam_macros::test]
    async fn full_flow__correct_credential__keys_should_match(ctx: &mut Context) -> Result<()> {
        let vault = Vault::create();

        let key_exchanger = X3dhNewKeyExchanger::new(vault.async_try_clone().await?);

        let mut initiator = key_exchanger.initiator().await?;
        let mut responder = key_exchanger.responder().await?;

        loop {
            if !initiator.is_complete().await? {
                let m = initiator.generate_request(&[]).await?;
                let _ = responder.handle_response(&m).await?;
            }

            if !responder.is_complete().await? {
                let m = responder.generate_request(&[]).await?;
                let _ = initiator.handle_response(&m).await?;
            }

            if initiator.is_complete().await? && responder.is_complete().await? {
                break;
            }
        }

        let initiator = initiator.finalize().await?;
        let responder = responder.finalize().await?;

        assert_eq!(initiator.h(), responder.h());

        let s1 = vault.secret_export(initiator.encrypt_key()).await?;
        let s2 = vault.secret_export(responder.decrypt_key()).await?;

        assert_eq!(s1, s2);

        let s1 = vault.secret_export(initiator.decrypt_key()).await?;
        let s2 = vault.secret_export(responder.encrypt_key()).await?;

        assert_eq!(s1, s2);
        ctx.stop().await
    }
}
