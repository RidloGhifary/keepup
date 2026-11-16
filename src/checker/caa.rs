use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;
use std::time::Duration;

use hickory_resolver::config::{NameServerConfig, ResolverConfig, ResolverOpts};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::proto::rr::rdata::CAA;
use hickory_resolver::proto::rr::{Name, RData, RecordType};
use hickory_resolver::TokioResolver;
use serde::{Deserialize, Serialize};

/// Result of a DNS CAA authorization inspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaaValidationResult {
    /// "VALID" | "UNRESTRICTED" | "MISMATCH"
    pub status: String,
    /// List of authorized CA domain identifiers found in CAA records.
    pub authorized_cas: Vec<String>,
    /// Human-readable explanation and matching summary.
    pub details: String,
}

/// Clean raw hostname or URL string for CAA DNS query.
pub fn clean_domain_for_caa(target: &str) -> String {
    let mut s = target.trim();
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
    if let Some(pos) = s.find(':') {
        s = &s[..pos];
    }
    s.trim().trim_end_matches('.').to_lowercase()
}

/// Helper to build a TokioResolver for CAA queries with reasonable timeouts.
fn build_caa_resolver(timeout_secs: u64) -> Result<TokioResolver, String> {
    let mut opts = ResolverOpts::default();
    opts.timeout = Duration::from_secs(timeout_secs.max(2));
    opts.attempts = 1;

    let builder = match TokioResolver::builder_tokio() {
        Ok(b) => b.with_options(opts),
        Err(_) => {
            let ns1 = NameServerConfig::udp_and_tcp(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));
            let ns2 = NameServerConfig::udp_and_tcp(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)));
            let cfg = ResolverConfig::from_name_servers(vec![ns1, ns2]);
            TokioResolver::builder_with_config(cfg, TokioRuntimeProvider::default())
                .with_options(opts)
        }
    };

    builder
        .build()
        .map_err(|e| format!("Failed to build CAA resolver: {e}"))
}

/// Parse and extract authorized CA identifiers from CAA records.
/// Returns (authorized_cas, explicitly_forbids_all).
pub fn parse_caa_records(records: &[CAA]) -> (Vec<String>, bool) {
    let mut cas = Vec::new();
    let mut forbids_all = false;

    for caa in records {
        let tag = caa.tag.to_string().to_lowercase();
        if tag == "issue" || tag == "issuewild" {
            let val_raw = String::from_utf8_lossy(&caa.value);
            let val = val_raw.trim();
            // Value format may be: "letsencrypt.org", "digicert.com; validationmethods=dns-01", or ";"
            let ca_token = val
                .split(|c: char| c == ';' || c.is_whitespace())
                .next()
                .unwrap_or("")
                .trim()
                .to_lowercase();

            if ca_token.is_empty() {
                // An empty issue tag (like `issue ";"`) explicitly denies certificate issuance for all CAs
                if val.starts_with(';') || val.is_empty() {
                    forbids_all = true;
                }
            } else if !cas.contains(&ca_token) {
                cas.push(ca_token);
            }
        }
    }

    (cas, forbids_all)
}

/// Test whether an issuer identity matches an authorized CA domain string.
pub fn matches_authorized_ca(
    issuer_cn: Option<&str>,
    issuer_org: Option<&str>,
    issuer_full: &str,
    ca_domain: &str,
) -> bool {
    let text = format!(
        "{} {} {}",
        issuer_cn.unwrap_or(""),
        issuer_org.unwrap_or(""),
        issuer_full
    )
    .to_lowercase();

    let ca = ca_domain.trim().to_lowercase();

    // 1. Direct domain or base substring match
    if text.contains(&ca) {
        return true;
    }

    // 2. Well-known CA domain to brand / root mapping
    match ca.as_str() {
        "letsencrypt.org" => {
            text.contains("let's encrypt")
                || text.contains("letsencrypt")
                || text.contains("isrg")
                || text.contains("internet security research group")
        }
        "digicert.com" => {
            text.contains("digicert")
                || text.contains("encryption everywhere")
                || text.contains("rapidssl")
                || text.contains("geotrust")
                || text.contains("thawte")
        }
        "sectigo.com" | "comodoca.com" => {
            text.contains("sectigo") || text.contains("comodo") || text.contains("usertrust")
        }
        "amazon.com" | "amazontrust.com" => text.contains("amazon") || text.contains("aws"),
        "google.com" | "pki.goog" => {
            text.contains("google trust services")
                || text.contains("gts")
                || text.contains("google")
        }
        "globalsign.com" => text.contains("globalsign"),
        "cloudflare.com" => text.contains("cloudflare"),
        "buypass.com" => text.contains("buypass"),
        "zerossl.com" => text.contains("zerossl"),
        "godaddy.com" => text.contains("godaddy") || text.contains("starfield"),
        "entrust.net" => text.contains("entrust"),
        "identrust.com" => text.contains("identrust") || text.contains("trustid"),
        "quovadisglobal.com" => text.contains("quovadis"),
        "camerfirma.com" => text.contains("camerfirma"),
        _ => {
            let base = ca.split('.').next().unwrap_or(ca.as_str());
            base.len() >= 3 && text.contains(base)
        }
    }
}

/// Evaluates CAA compliance given the active certificate issuer and the list of CAA records.
pub fn evaluate_caa_compliance(
    issuer_cn: Option<&str>,
    issuer_org: Option<&str>,
    issuer_full: &str,
    records: &[CAA],
) -> CaaValidationResult {
    if records.is_empty() {
        return CaaValidationResult {
            status: "UNRESTRICTED".to_string(),
            authorized_cas: Vec::new(),
            details: "No CAA records configured; any valid CA permitted".to_string(),
        };
    }

    let (authorized_cas, forbids_all) = parse_caa_records(records);

    if forbids_all && authorized_cas.is_empty() {
        return CaaValidationResult {
            status: "MISMATCH".to_string(),
            authorized_cas,
            details: "Domain CAA explicitly forbids all certificate issuance (issue ;)".to_string(),
        };
    }

    if authorized_cas.is_empty() {
        return CaaValidationResult {
            status: "UNRESTRICTED".to_string(),
            authorized_cas,
            details: "No issue/issuewild tags found; all public CAs permitted".to_string(),
        };
    }

    let issuer_summary = issuer_cn.or(issuer_org).unwrap_or(issuer_full);

    // Cross-reference issuer against authorized CAs
    for ca in &authorized_cas {
        if matches_authorized_ca(issuer_cn, issuer_org, issuer_full, ca) {
            return CaaValidationResult {
                status: "VALID".to_string(),
                authorized_cas: authorized_cas.clone(),
                details: format!(
                    "Authorized CA: {} (matched {})",
                    authorized_cas.join(", "),
                    ca
                ),
            };
        }
    }

    CaaValidationResult {
        status: "MISMATCH".to_string(),
        authorized_cas: authorized_cas.clone(),
        details: format!(
            "Certificate issuer '{}' not permitted by CAA policy (authorized: {})",
            issuer_summary,
            authorized_cas.join(", ")
        ),
    }
}

/// Query CAA records for target hostname, performing parent domain fallback until records are found or root is reached.
pub async fn inspect_caa_records(
    target_host: &str,
    issuer_cn: Option<&str>,
    issuer_org: Option<&str>,
    issuer_full: &str,
    timeout_secs: u64,
) -> CaaValidationResult {
    let domain = clean_domain_for_caa(target_host);
    if domain.is_empty() {
        return CaaValidationResult {
            status: "UNRESTRICTED".to_string(),
            authorized_cas: Vec::new(),
            details: "Target domain is empty".to_string(),
        };
    }

    let resolver = match build_caa_resolver(timeout_secs) {
        Ok(r) => r,
        Err(e) => {
            return CaaValidationResult {
                status: "UNRESTRICTED".to_string(),
                authorized_cas: Vec::new(),
                details: format!("DNS CAA lookup skipped: {e}"),
            };
        }
    };

    // Walk up the domain hierarchy: e.g. api.sub.example.com -> sub.example.com -> example.com
    let mut current_domain = domain.clone();
    let mut seen_domains = HashSet::new();

    loop {
        if !seen_domains.insert(current_domain.clone()) {
            break;
        }

        if let Ok(name) = Name::from_str(&current_domain) {
            if let Ok(response) = resolver.lookup(name, RecordType::CAA).await {
                let mut caa_records = Vec::new();
                for ans in response.answers() {
                    if let RData::CAA(caa) = &ans.data {
                        caa_records.push(caa.clone());
                    }
                }

                if !caa_records.is_empty() {
                    return evaluate_caa_compliance(
                        issuer_cn,
                        issuer_org,
                        issuer_full,
                        &caa_records,
                    );
                }
            }
        }

        // Move to parent domain: split first component off
        if let Some(pos) = current_domain.find('.') {
            let parent = &current_domain[pos + 1..];
            if parent.contains('.') {
                current_domain = parent.to_string();
            } else {
                break;
            }
        } else {
            break;
        }
    }

    // No CAA records found anywhere in hierarchy
    CaaValidationResult {
        status: "UNRESTRICTED".to_string(),
        authorized_cas: Vec::new(),
        details: "No CAA records configured; any valid CA permitted".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mock_caa(tag: &str, value: &str) -> CAA {
        let name = if value == ";" || value.is_empty() {
            None
        } else {
            Name::from_str(value).ok()
        };
        if tag == "issuewild" {
            CAA::new_issuewild(false, name, vec![])
        } else {
            CAA::new_issue(false, name, vec![])
        }
    }

    #[test]
    fn test_clean_domain_for_caa() {
        assert_eq!(
            clean_domain_for_caa("https://api.example.com:8443/health?q=1"),
            "api.example.com"
        );
        assert_eq!(clean_domain_for_caa("example.com."), "example.com");
        assert_eq!(
            clean_domain_for_caa("HTTP://ONE.ONE.ONE.ONE/"),
            "one.one.one.one"
        );
    }

    #[test]
    fn test_caa_unrestricted_when_no_records() {
        let res = evaluate_caa_compliance(
            Some("R10"),
            Some("Let's Encrypt"),
            "CN=R10, O=Let's Encrypt, C=US",
            &[],
        );
        assert_eq!(res.status, "UNRESTRICTED");
        assert!(res.details.contains("No CAA records"));
        assert!(res.authorized_cas.is_empty());
    }

    #[test]
    fn test_caa_valid_letsencrypt() {
        let caa_list = vec![
            make_mock_caa("issue", "letsencrypt.org"),
            make_mock_caa("issue", "digicert.com"),
        ];

        let res = evaluate_caa_compliance(
            Some("R10"),
            Some("Let's Encrypt"),
            "CN=R10, O=Let's Encrypt, C=US",
            &caa_list,
        );
        assert_eq!(res.status, "VALID");
        assert!(res.details.contains("Authorized CA"));
        assert_eq!(res.authorized_cas, vec!["letsencrypt.org", "digicert.com"]);
    }

    #[test]
    fn test_caa_valid_google_pki() {
        let caa_list = vec![make_mock_caa("issue", "pki.goog")];

        let res = evaluate_caa_compliance(
            Some("GTS CA 1C3"),
            Some("Google Trust Services LLC"),
            "CN=GTS CA 1C3, O=Google Trust Services LLC, C=US",
            &caa_list,
        );
        assert_eq!(res.status, "VALID");
    }

    #[test]
    fn test_caa_mismatch() {
        let caa_list = vec![make_mock_caa("issue", "digicert.com")];

        let res = evaluate_caa_compliance(
            Some("R10"),
            Some("Let's Encrypt"),
            "CN=R10, O=Let's Encrypt, C=US",
            &caa_list,
        );
        assert_eq!(res.status, "MISMATCH");
        assert!(res.details.contains("not permitted by CAA policy"));
        assert_eq!(res.authorized_cas, vec!["digicert.com"]);
    }

    #[test]
    fn test_caa_explicit_forbid_all() {
        let caa_list = vec![make_mock_caa("issue", ";")];

        let res = evaluate_caa_compliance(
            Some("R10"),
            Some("Let's Encrypt"),
            "CN=R10, O=Let's Encrypt, C=US",
            &caa_list,
        );
        assert_eq!(res.status, "MISMATCH");
        assert!(res.details.contains("forbids all"));
    }
}
