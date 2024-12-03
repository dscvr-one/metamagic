/// Adapted from ic-validator found here: https://github.com/dfinity/ic/blob/0a51fd74f08b2e6f23d6e1d60f1f52eb73b40ccc/rs/validator/src/ingress_validation.rs
///
/// Rather than validating an HttpRequest, we will validate the delegation found in a JWT.  This still
/// requires a user to setup a ingress verifier as before against a URL with a trust provider.
use crate::webauthn::validate_webauthn_sig;
use ic_agent::agent::http_transport::ReqwestTransport;
use ic_agent::export::Principal;
use ic_agent::identity::AnonymousIdentity;
use ic_agent::Agent;
use ic_constants::{MAX_INGRESS_TTL, PERMITTED_DRIFT_AT_VALIDATOR};
use ic_crypto_interfaces_sig_verification::IngressSigVerifier;
use ic_crypto_standalone_sig_verifier::{user_public_key_from_bytes, KeyBytesContentType};
use ic_crypto_utils_threshold_sig_der::parse_threshold_sig_key_from_der;
use ic_types::crypto::threshold_sig::{IcRootOfTrust, RootOfTrustProvider};
use ic_types::crypto::{CanisterSig, CanisterSigOf};
use ic_types::messages::Blob;
use ic_types::{
    crypto::{BasicSig, BasicSigOf, CryptoError, UserPublicKey},
    messages::{Delegation, SignedDelegation, WebAuthnSignature},
    CanisterId, PrincipalId, Time, UserId,
};
use ic_validator::CanisterIdSet;
use ic_validator_ingress_message::StandaloneIngressSigVerifier;
use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::Arc;
use std::{convert::TryFrom, fmt};
use AuthenticationError::*;
use RequestValidationError::*;

/// Maximum number of targets (collection of `CanisterId`s) that can be specified in a
/// single delegation. Requests having a single delegation with more targets will be declared
/// invalid without any further verification.
/// **Note**: this limit is part of the [IC specification](https://internetcomputer.org/docs/current/references/ic-interface-spec#authentication)
/// and so changing this value might be breaking or result in a deviation from the specification.
const MAXIMUM_NUMBER_OF_TARGETS_PER_DELEGATION: usize = 1_000;

pub struct Head {
    pub issuer: Principal,
    pub issued_at: u64,
    pub expires_at: u64,
}

pub struct Body {
    delegation: SignedDelegation,
    sender_pub_key: Blob,
}

pub struct Token {
    head: Head,
    body: Body,
    // Base64 ecndoded bytes of signature(private_key, (self.head, self.body))
    signature: Blob,
}

pub struct ConstantRootOfTrustProvider {
    root_of_trust: IcRootOfTrust,
}

impl ConstantRootOfTrustProvider {
    fn new(root_of_trust: IcRootOfTrust) -> Self {
        Self { root_of_trust }
    }
}

impl RootOfTrustProvider for ConstantRootOfTrustProvider {
    type Error = Infallible;

    fn root_of_trust(&self) -> Result<IcRootOfTrust, Self::Error> {
        Ok(self.root_of_trust)
    }
}

pub struct TokenValidator {
    signature_verifier: StandaloneIngressSigVerifier,
    trust_provider: ConstantRootOfTrustProvider,
}

impl TokenValidator {
    pub async fn try_new(url: &str) -> instrumented_error::Result<Self> {
        let agent: Agent = Agent::builder()
            .with_transport(ReqwestTransport::create(url)?)
            .with_arc_identity(Arc::new(AnonymousIdentity))
            .build()?;
        agent.fetch_root_key().await?;
        let public_key = parse_threshold_sig_key_from_der(&agent.read_root_key())?;

        Ok(Self {
            signature_verifier: StandaloneIngressSigVerifier,
            trust_provider: ConstantRootOfTrustProvider::new(public_key.into()),
        })
    }

    pub fn validate_token(&self, token: Token) -> instrumented_error::Result<Principal> {
        let current_time = Time::from_nanos_since_unix_epoch(0);
        validate_ingress_expiry(token.head.expires_at, current_time)?;
        validate_delegations(
            &self.signature_verifier,
            vec![token.body.delegation.clone()].as_slice(),
            token.body.sender_pub_key.0.clone(),
            &self.trust_provider,
        )?;
        validate_token_user_id_and_signature(&token)?;
        Ok(token.head.issuer)
    }
}

/// Error in validating an [HttpRequest].
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum RequestValidationError {
    InvalidIngressExpiry(String),
    InvalidDelegationExpiry(String),
    UserIdDoesNotMatchPublicKey(UserId, Vec<u8>),
    InvalidSignature(AuthenticationError),
    InvalidDelegation(AuthenticationError),
    MissingSignature(UserId),
    AnonymousSignatureNotAllowed,
    CanisterNotInDelegationTargets(CanisterId),
    TooManyPathsError { length: usize, maximum: usize },
    PathTooLongError { length: usize, maximum: usize },
    NonceTooBigError { num_bytes: usize, maximum: usize },
}

impl fmt::Display for RequestValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvalidIngressExpiry(msg) => write!(f, "{}", msg),
            InvalidDelegationExpiry(msg) => write!(f, "{}", msg),
            UserIdDoesNotMatchPublicKey(user_id, pubkey) => write!(
                f,
                "The user id {} does not match the public key {}",
                user_id,
                hex::encode(pubkey)
            ),
            InvalidSignature(err) => write!(f, "Invalid signature: {}", err),
            InvalidDelegation(err) => write!(f, "Invalid delegation: {}", err),
            MissingSignature(user_id) => write!(f, "Missing signature from user: {}", user_id),
            AnonymousSignatureNotAllowed => {
                write!(f, "Signature is not allowed for the anonymous user")
            }
            CanisterNotInDelegationTargets(canister_id) => write!(
                f,
                "Canister {} is not one of the delegation targets",
                canister_id
            ),
            TooManyPathsError { length, maximum } => write!(
                f,
                "Too many paths in read state request: got {} paths, but at most {} are allowed",
                length, maximum
            ),
            PathTooLongError { length, maximum } => write!(
                f,
                "At least one path in read state request is too deep: got {} labels, but at most {} are allowed",
                length, maximum
            ),
            NonceTooBigError { num_bytes: length, maximum } => write!(
                f,
                "Nonce in request is too big: got {} bytes, but at most {} are allowed",
                length, maximum
            ),
        }
    }
}

/// Error in verifying the signature or authentication part of a request.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum AuthenticationError {
    InvalidBasicSignature(CryptoError),
    InvalidCanisterSignature(String),
    InvalidPublicKey(CryptoError),
    WebAuthnError(String),
    DelegationTargetError(String),
    DelegationTooLongError { length: usize, maximum: usize },
    DelegationContainsCyclesError { public_key: Vec<u8> },
}

impl fmt::Display for AuthenticationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvalidBasicSignature(err) => write!(f, "Invalid basic signature: {}", err),
            InvalidCanisterSignature(err) => write!(f, "Invalid canister signature: {}", err),
            InvalidPublicKey(err) => write!(f, "Invalid public key: {}", err),
            WebAuthnError(msg) => write!(f, "{}", msg),
            DelegationTargetError(msg) => write!(f, "{}", msg),
            DelegationTooLongError { length, maximum } => write!(
                f,
                "Chain of delegations is too long: got {} delegations, but at most {} are allowed",
                length, maximum
            ),
            DelegationContainsCyclesError { public_key } => write!(
                f,
                "Chain of delegations contains at least one cycle: first repeating public key encountered {}",
                hex::encode(public_key)
            ),
        }
    }
}

// Check if ingress_expiry is within a proper range with respect to the given
// time, i.e., it is not expired yet and is not too far in the future.
fn validate_ingress_expiry(
    request_expiry: u64,
    current_time: Time,
) -> Result<(), RequestValidationError> {
    let provided_expiry = Time::from_nanos_since_unix_epoch(request_expiry);
    let min_allowed_expiry = current_time;
    // We need to account for time drift and be more forgiving at rejecting ingress
    // messages due to their expiry being too far in the future.
    let max_expiry_diff = MAX_INGRESS_TTL
        .checked_add(PERMITTED_DRIFT_AT_VALIDATOR)
        .ok_or_else(|| {
            InvalidIngressExpiry(format!(
                "Addition of MAX_INGRESS_TTL {MAX_INGRESS_TTL:?} with \
                PERMITTED_DRIFT_AT_VALIDATOR {PERMITTED_DRIFT_AT_VALIDATOR:?} overflows",
            ))
        })?;
    let max_allowed_expiry = min_allowed_expiry
        .checked_add(max_expiry_diff)
        .ok_or_else(|| {
            InvalidIngressExpiry(format!(
                "Addition of min_allowed_expiry {min_allowed_expiry:?} \
                with max_expiry_diff {max_expiry_diff:?} overflows",
            ))
        })?;
    if !(min_allowed_expiry <= provided_expiry && provided_expiry <= max_allowed_expiry) {
        let msg = format!(
            "Specified ingress_expiry not within expected range: \
             Minimum allowed expiry: {}, \
             Maximum allowed expiry: {}, \
             Provided expiry:        {}",
            min_allowed_expiry, max_allowed_expiry, provided_expiry
        );
        return Err(InvalidIngressExpiry(msg));
    }
    Ok(())
}

// Verifies that the user id matches the public key.  Returns an error if not.
fn validate_user_id(sender_pubkey: &[u8], id: PrincipalId) -> Result<(), RequestValidationError> {
    if id == PrincipalId::new_self_authenticating(sender_pubkey) {
        Ok(())
    } else {
        Err(UserIdDoesNotMatchPublicKey(
            UserId::new(id),
            sender_pubkey.to_vec(),
        ))
    }
}

// Validate a chain of delegations.
// See https://internetcomputer.org/docs/current/references/ic-interface-spec#authentication
//
// If the delegations are valid, returns the public key used to sign the
// request as well as the set of canister IDs that the public key is valid for.
fn validate_delegations<R: RootOfTrustProvider>(
    validator: &dyn IngressSigVerifier,
    signed_delegations: &[SignedDelegation],
    pubkey: Vec<u8>,
    root_of_trust_provider: &R,
) -> Result<(), RequestValidationError>
where
    R::Error: std::error::Error,
{
    ensure_delegations_does_not_contain_cycles(&pubkey, signed_delegations)?;
    ensure_delegations_does_not_contain_too_many_targets(signed_delegations)?;

    for sd in signed_delegations {
        let delegation = sd.delegation();
        let signature = sd.signature();

        validate_delegation(
            validator,
            signature,
            delegation,
            &pubkey,
            root_of_trust_provider,
        )
        .map_err(InvalidDelegation)?;
    }
    Ok(())
}

fn ensure_delegations_does_not_contain_cycles(
    sender_public_key: &[u8],
    signed_delegations: &[SignedDelegation],
) -> Result<(), RequestValidationError> {
    let mut observed_public_keys = HashSet::with_capacity(signed_delegations.len() + 1);
    observed_public_keys.insert(sender_public_key);
    for delegation in signed_delegations {
        let current_public_key = delegation.delegation().pubkey();
        if !observed_public_keys.insert(current_public_key) {
            return Err(InvalidDelegation(DelegationContainsCyclesError {
                public_key: current_public_key.clone(),
            }));
        }
    }
    Ok(())
}

fn ensure_delegations_does_not_contain_too_many_targets(
    signed_delegations: &[SignedDelegation],
) -> Result<(), RequestValidationError> {
    for delegation in signed_delegations {
        match delegation.delegation().number_of_targets() {
            Some(number_of_targets)
                if number_of_targets > MAXIMUM_NUMBER_OF_TARGETS_PER_DELEGATION =>
            {
                Err(InvalidDelegation(DelegationTargetError(format!(
                    "expected at most {} targets per delegation, but got {}",
                    MAXIMUM_NUMBER_OF_TARGETS_PER_DELEGATION, number_of_targets
                ))))
            }
            _ => Ok(()),
        }?
    }
    Ok(())
}

fn validate_delegation<R: RootOfTrustProvider>(
    validator: &dyn IngressSigVerifier,
    signature: &[u8],
    delegation: &Delegation,
    pubkey: &[u8],
    root_of_trust_provider: &R,
) -> Result<CanisterIdSet, AuthenticationError>
where
    R::Error: std::error::Error,
{
    let (pk, pk_type) = public_key_from_bytes(pubkey)?;

    match pk_type {
        KeyBytesContentType::EcdsaP256PublicKeyDerWrappedCose
        | KeyBytesContentType::RsaSha256PublicKeyDerWrappedCose => {
            let webauthn_sig = WebAuthnSignature::try_from(signature).map_err(WebAuthnError)?;
            validate_webauthn_sig(validator, &webauthn_sig, delegation, &pk)
                .map_err(WebAuthnError)?;
        }
        KeyBytesContentType::Ed25519PublicKeyDer
        | KeyBytesContentType::EcdsaP256PublicKeyDer
        | KeyBytesContentType::EcdsaSecp256k1PublicKeyDer
        | KeyBytesContentType::RsaSha256PublicKeyDer => {
            let basic_sig = BasicSigOf::from(BasicSig(signature.to_vec()));
            validator
                .verify_basic_sig_by_public_key(&basic_sig, delegation, &pk)
                .map_err(InvalidBasicSignature)?;
        }
        KeyBytesContentType::IcCanisterSignatureAlgPublicKeyDer => {
            let canister_sig = CanisterSigOf::from(CanisterSig(signature.to_vec()));
            let root_of_trust = root_of_trust_provider
                .root_of_trust()
                .map_err(|e| InvalidCanisterSignature(e.to_string()))?;
            validator
                .verify_canister_sig(&canister_sig, delegation, &pk, &root_of_trust)
                .map_err(|e| InvalidCanisterSignature(e.to_string()))?;
        }
    }

    // Validation succeeded. Return the targets of this delegation.
    Ok(match delegation.targets().map_err(DelegationTargetError)? {
        None => CanisterIdSet::all(),
        Some(targets) => CanisterIdSet::try_from_iter(targets)
            .map_err(|e| DelegationTargetError(format!("{e}")))?,
    })
}

fn validate_token_user_id_and_signature(token: &Token) -> Result<(), RequestValidationError> {
    let _signature = &token.signature;
    let id = PrincipalId::from(token.head.issuer);
    if id.is_anonymous() {
        Err(AnonymousSignatureNotAllowed)
    } else {
        let sender_pubkey = &token.body.sender_pub_key.0;
        validate_user_id(sender_pubkey, id)?;
        Ok(())
    }
}

fn public_key_from_bytes(
    pubkey: &[u8],
) -> Result<(UserPublicKey, KeyBytesContentType), AuthenticationError> {
    user_public_key_from_bytes(pubkey).map_err(InvalidPublicKey)
}
