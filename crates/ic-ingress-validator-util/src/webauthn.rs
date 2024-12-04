use ic_crypto_interfaces_sig_verification::IngressSigVerifier;
use ic_crypto_standalone_sig_verifier::{
    ecdsa_p256_signature_from_der_bytes, rsa_signature_from_bytes,
};
use ic_types::{
    crypto::{AlgorithmId, BasicSig, BasicSigOf, Signable, UserPublicKey},
    messages::{WebAuthnEnvelope, WebAuthnSignature},
};
use std::convert::TryFrom;

/// Verifies that a `WebAuthnSignature` signs a `Signable`.
pub(crate) fn validate_webauthn_sig(
    verifier: &dyn IngressSigVerifier,
    webauthn_sig: &WebAuthnSignature,
    signable: &impl Signable,
    public_key: &UserPublicKey,
) -> Result<(), String> {
    let basic_sig = basic_sig_from_webauthn_sig(&webauthn_sig, public_key.algorithm_id)?;

    let envelope = WebAuthnEnvelope::try_from(webauthn_sig)
        .map_err(|err| format!("WebAuthn envelope creation failed: {}", err))?;

    // Verify the signature signs the `WebAuthnEnvelope` provided.
    verifier
        .verify_basic_sig_by_public_key(&BasicSigOf::from(basic_sig.clone()), &envelope, public_key)
        .map_err(|e| {
            format!(
                "Verifying signature failed. signature: {:?}; envelope: {:?}; public_key: {}. Error: {}",
                basic_sig, envelope.clone(), public_key, e
            )
        })?;

    // The challenge in the webauthn envelope must match signed bytes.
    let signed_bytes = signable.as_signed_bytes();
    if envelope.challenge() != signed_bytes {
        Err(format!(
            "Challenge in webauthn is {:?} while it is expected to be {:?}",
            envelope.challenge(),
            signed_bytes,
        ))
    } else {
        Ok(())
    }
}

fn basic_sig_from_webauthn_sig(
    webauthn_sig: &WebAuthnSignature,
    algorithm_id: AlgorithmId,
) -> Result<BasicSig, String> {
    match algorithm_id {
        AlgorithmId::EcdsaP256 => {
            // ECDSA signatures are DER wrapped, see https://www.w3.org/TR/webauthn-2/#sctn-signature-attestation-types
            ecdsa_p256_signature_from_der_bytes(&webauthn_sig.signature().0)
                .map_err(|e| format!("Failed to parse EcdsaP256 signature: {}", e))
        }
        AlgorithmId::RsaSha256 => {
            // RSA signatures are not DER wrapped, see https://www.w3.org/TR/webauthn-2/#sctn-signature-attestation-types
            Ok(rsa_signature_from_bytes(&webauthn_sig.signature()))
        }
        _ => Err(format!(
            "Only ECDSA on curve P-256 and RSA PKCS #1 v1.5 are supported for WebAuthn, given: {:?}",
            algorithm_id
        ))
    }
}
