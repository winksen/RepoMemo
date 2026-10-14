//! Outgoing mail for RepoMemo: composing a plain-text message and handing it
//! to an SMTP server over TLS (roadmap F6a, V0.2.9).
//!
//! The crate knows nothing of the server's database or queue: it takes
//! settings, a policy for which addresses may be called, and one message, and
//! says whether the message was accepted and, when it was not, whether trying
//! again can help.

mod compose;
mod smtp;

#[cfg(test)]
mod test_certificates;

#[cfg(test)]
mod tests;

pub use compose::{compose, valid_address, ComposeError, Outgoing};
pub use smtp::{check_host, send, OutboundPolicy, Security, SendError, SmtpSettings};

/// The certificates (DER) in a PEM bundle, for `OutboundPolicy::extra_roots`.
pub fn parse_pem_certificates(pem: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    use rustls_pki_types::pem::PemObject;
    let certificates = rustls_pki_types::CertificateDer::pem_slice_iter(pem)
        .map(|certificate| certificate.map(|der| der.to_vec()).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    if certificates.is_empty() {
        return Err("no certificate was found in it".to_owned());
    }
    Ok(certificates)
}
