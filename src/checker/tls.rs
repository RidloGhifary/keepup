use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Utc};
use rustls_pki_types::ServerName;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use tokio_rustls::rustls::{ClientConfig, DigitallySignedStruct, Error as RustlsError, SignatureScheme};
use tokio_rustls::TlsConnector;
use x509_parser::prelude::*;

/// Output of a TLS / SSL certificate inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SslExpiryInfo {
    pub host: String,
    pub expires_at: String,
    pub days_remaining: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum SslCheckError {
    #[error("Invalid host: {0}")]
    InvalidHost(String),
    #[error("Connection error: {0}")]
    Connect(String),
    #[error("TLS handshake error: {0}")]
    Handshake(String),
    #[error("No certificate presented")]
    NoCertificates,
    #[error("Failed to parse certificate: {0}")]
    Parse(String),
    #[error("Timeout checking TLS certificate")]
    Timeout,
}

#[derive(Debug)]
struct DangerCertVerifier;

impl ServerCertVerifier for DangerCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls_pki_types::CertificateDer<'_>,
        _intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls_pki_types::UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        tokio_rustls::rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Extracts (host, port) from a raw URL or hostname string.
pub fn extract_host_port(raw_url: &str) -> Result<(String, u16), SslCheckError> {
    let mut s = raw_url.trim();
    if let Some(pos) = s.find("://") {
        s = &s[pos + 3..];
    }
    if let Some(pos) = s.find('/') {
        s = &s[..pos];
    }
    if let Some(pos) = s.find('?') {
        s = &s[..pos];
    }
    if let Some(pos) = s.find('#') {
        s = &s[..pos];
    }
    if let Some(pos) = s.rfind('@') {
        s = &s[pos + 1..];
    }

    let parts: Vec<&str> = s.split(':').collect();
    if parts.is_empty() || parts[0].trim().is_empty() {
        return Err(SslCheckError::InvalidHost(raw_url.to_string()));
    }

    let host = parts[0].trim().to_lowercase();
    let port = if parts.len() > 1 {
        parts[1].parse::<u16>().unwrap_or(443)
    } else {
        443
    };

    Ok((host, port))
}

/// Connects to the host via TLS, retrieves the server certificate, and extracts expiration details.
pub async fn check_ssl_expiration(raw_url: &str, timeout_secs: u64) -> Result<SslExpiryInfo, SslCheckError> {
    let (host, port) = extract_host_port(raw_url)?;

    timeout(Duration::from_secs(timeout_secs), async {
        let server_name = ServerName::try_from(host.clone())
            .map_err(|e| SslCheckError::InvalidHost(format!("{host}: {e}")))?;

        let config = ClientConfig::builder_with_provider(Arc::new(
            tokio_rustls::rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|e| SslCheckError::Handshake(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(DangerCertVerifier))
        .with_no_client_auth();

        let connector = TlsConnector::from(Arc::new(config));

        let tcp_stream = TcpStream::connect((host.as_str(), port))
            .await
            .map_err(|e| SslCheckError::Connect(e.to_string()))?;

        let tls_stream = connector
            .connect(server_name, tcp_stream)
            .await
            .map_err(|e| SslCheckError::Handshake(e.to_string()))?;

        let peer_certs = tls_stream
            .get_ref()
            .1
            .peer_certificates()
            .ok_or(SslCheckError::NoCertificates)?;

        let leaf_der = peer_certs.first().ok_or(SslCheckError::NoCertificates)?;

        let (_, cert) = parse_x509_certificate(leaf_der.as_ref())
            .map_err(|e| SslCheckError::Parse(e.to_string()))?;

        let not_after = cert.validity().not_after;
        let not_after_timestamp = not_after.timestamp();
        let not_after_dt = DateTime::<Utc>::from_timestamp(not_after_timestamp, 0)
            .ok_or_else(|| SslCheckError::Parse("Invalid timestamp".to_string()))?;

        let now = Utc::now();
        let duration = not_after_dt.signed_duration_since(now);
        let days_remaining = duration.num_days();

        Ok(SslExpiryInfo {
            host,
            expires_at: not_after_dt.to_rfc3339(),
            days_remaining,
        })
    })
    .await
    .map_err(|_| SslCheckError::Timeout)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_host_port() {
        assert_eq!(extract_host_port("https://example.com").unwrap(), ("example.com".to_string(), 443));
        assert_eq!(extract_host_port("https://example.com:8443/path?q=1").unwrap(), ("example.com".to_string(), 8443));
        assert_eq!(extract_host_port("http://api.sub.domain.org/health").unwrap(), ("api.sub.domain.org".to_string(), 443));
        assert_eq!(extract_host_port("internal.service:9000").unwrap(), ("internal.service".to_string(), 9000));
    }
}
