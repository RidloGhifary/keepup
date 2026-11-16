use chrono::{DateTime, Utc};
use rustls_pki_types::ServerName;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::{
    ClientConfig, DigitallySignedStruct, Error as RustlsError, SignatureScheme,
};
use tokio_rustls::TlsConnector;
use x509_parser::extensions::GeneralName;
use x509_parser::prelude::*;

/// Output of a TLS / SSL certificate inspection.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SslExpiryInfo {
    pub host: String,
    pub issued_at: Option<String>,
    pub expires_at: String,
    pub days_remaining: i64,
    pub issuer_cn: Option<String>,
    pub issuer_org: Option<String>,
    pub issuer_full: String,
    pub serial_number: Option<String>,
    pub signature_algorithm: Option<String>,
    pub sans: Vec<String>,
}

fn format_signature_algorithm(oid_str: &str) -> String {
    match oid_str {
        "1.2.840.113549.1.1.11" => "sha256WithRSAEncryption".to_string(),
        "1.2.840.113549.1.1.12" => "sha384WithRSAEncryption".to_string(),
        "1.2.840.113549.1.1.13" => "sha512WithRSAEncryption".to_string(),
        "1.2.840.113549.1.1.14" => "sha224WithRSAEncryption".to_string(),
        "1.2.840.113549.1.1.5" => "sha1WithRSAEncryption".to_string(),
        "1.2.840.10045.4.3.2" => "ecdsa-with-SHA256".to_string(),
        "1.2.840.10045.4.3.3" => "ecdsa-with-SHA384".to_string(),
        "1.2.840.10045.4.3.4" => "ecdsa-with-SHA512".to_string(),
        "1.3.101.112" => "Ed25519".to_string(),
        "1.3.101.113" => "Ed448".to_string(),
        other => other.to_string(),
    }
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
pub async fn check_ssl_expiration(
    raw_url: &str,
    timeout_secs: u64,
) -> Result<SslExpiryInfo, SslCheckError> {
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

        let not_before = cert.validity().not_before;
        let not_before_dt = DateTime::<Utc>::from_timestamp(not_before.timestamp(), 0);
        let issued_at = not_before_dt.map(|dt| dt.to_rfc3339());

        let not_after = cert.validity().not_after;
        let not_after_timestamp = not_after.timestamp();
        let not_after_dt = DateTime::<Utc>::from_timestamp(not_after_timestamp, 0)
            .ok_or_else(|| SslCheckError::Parse("Invalid timestamp".to_string()))?;

        let now = Utc::now();
        let duration = not_after_dt.signed_duration_since(now);
        let days_remaining = duration.num_days();

        let issuer_cn = cert
            .issuer()
            .iter_common_name()
            .next()
            .and_then(|a| a.as_str().ok())
            .map(|s| s.to_string());

        let issuer_org = cert
            .issuer()
            .iter_organization()
            .next()
            .and_then(|a| a.as_str().ok())
            .map(|s| s.to_string());

        let issuer_full = cert.issuer().to_string();

        let serial_number = Some(cert.raw_serial_as_string());

        let sig_oid = cert.signature_algorithm.algorithm.to_id_string();
        let signature_algorithm = Some(format_signature_algorithm(&sig_oid));

        let mut sans = Vec::new();
        if let Ok(Some(san_ext)) = cert.subject_alternative_name() {
            for name in &san_ext.value.general_names {
                match name {
                    GeneralName::DNSName(s) => sans.push(s.to_string()),
                    GeneralName::IPAddress(b) => {
                        if b.len() == 4 {
                            sans.push(format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]));
                        } else if b.len() == 16 {
                            let mut segments = Vec::new();
                            for chunk in b.chunks(2) {
                                segments.push(format!(
                                    "{:x}",
                                    u16::from_be_bytes([chunk[0], chunk[1]])
                                ));
                            }
                            sans.push(segments.join(":"));
                        }
                    }
                    GeneralName::RFC822Name(s) => sans.push(s.to_string()),
                    GeneralName::URI(s) => sans.push(s.to_string()),
                    _ => {}
                }
            }
        }

        Ok(SslExpiryInfo {
            host,
            issued_at,
            expires_at: not_after_dt.to_rfc3339(),
            days_remaining,
            issuer_cn,
            issuer_org,
            issuer_full,
            serial_number,
            signature_algorithm,
            sans,
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
        assert_eq!(
            extract_host_port("https://example.com").unwrap(),
            ("example.com".to_string(), 443)
        );
        assert_eq!(
            extract_host_port("https://example.com:8443/path?q=1").unwrap(),
            ("example.com".to_string(), 8443)
        );
        assert_eq!(
            extract_host_port("http://api.sub.domain.org/health").unwrap(),
            ("api.sub.domain.org".to_string(), 443)
        );
        assert_eq!(
            extract_host_port("internal.service:9000").unwrap(),
            ("internal.service".to_string(), 9000)
        );
    }
}
