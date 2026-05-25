//! TLS material loading. Produces a `rustls::ServerConfig` ready to plug into
//! a TLS acceptor.
//!
//! Two paths:
//!   - file-based: load a PEM cert chain + PEM key from disk
//!   - self-signed: generate a dev certificate in-memory at startup
//!
//! ACME / Let's Encrypt is intentionally deferred — see README.

use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::config::TlsMode;

pub fn build_server_config(mode: &TlsMode) -> Result<Option<Arc<ServerConfig>>> {
    let (certs, key) = match mode {
        TlsMode::Plain => return Ok(None),
        TlsMode::Files { cert, key } => load_files(cert, key)?,
        TlsMode::SelfSigned { dns_names } => generate_self_signed(dns_names)?,
    };

    let cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("rustls rejected the cert/key pair")?;
    Ok(Some(Arc::new(cfg)))
}

fn load_files(
    cert: &Path,
    key: &Path,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let cert_bytes = fs::read(cert).with_context(|| format!("read cert {}", cert.display()))?;
    let mut cr = BufReader::new(&cert_bytes[..]);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cr)
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("parse cert {}", cert.display()))?;
    if certs.is_empty() {
        bail!("no certificates found in {}", cert.display());
    }

    let key_bytes = fs::read(key).with_context(|| format!("read key {}", key.display()))?;
    let mut kr = BufReader::new(&key_bytes[..]);
    let key_der = rustls_pemfile::private_key(&mut kr)
        .with_context(|| format!("parse key {}", key.display()))?
        .ok_or_else(|| anyhow::anyhow!("no private key found in {}", key.display()))?;

    Ok((certs, key_der))
}

fn generate_self_signed(
    dns_names: &[String],
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let cert = rcgen::generate_simple_self_signed(dns_names.to_vec())
        .context("rcgen failed to produce a self-signed cert")?;
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der = PrivateKeyDer::Pkcs8(cert.key_pair.serialize_der().into());

    tracing::warn!(
        names = ?dns_names,
        "using self-signed certificate — browsers will warn. Configure Proxy.Cert + Proxy.Key for production."
    );
    Ok((vec![cert_der], key_der))
}
