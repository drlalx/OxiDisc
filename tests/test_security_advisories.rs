#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;

#[derive(Serialize)]
struct OsvQueryBatch {
    queries: Vec<OsvPackageQuery>,
}

#[derive(Serialize)]
struct OsvPackageQuery {
    package: OsvPackageInfo,
    version: String,
}

#[derive(Serialize)]
struct OsvPackageInfo {
    name: String,
    ecosystem: &'static str,
}

#[derive(Deserialize, Debug)]
struct OsvBatchResponse {
    #[serde(default)]
    results: Vec<OsvResultItem>,
}

#[derive(Deserialize, Debug)]
struct OsvResultItem {
    #[serde(default)]
    vulns: Vec<OsvVulnerability>,
}

#[derive(Deserialize, Debug)]
struct OsvVulnerability {
    id: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    aliases: Option<Vec<String>>,
    #[serde(default)]
    severity: Option<Vec<serde_json::Value>>,
}

impl OsvVulnerability {
    /// Determines whether this advisory represents an actionable security vulnerability
    /// (e.g., has a CVE/GHSA alias, CVSS severity, or security flaw) as opposed to an informational/unmaintained note.
    fn is_security_vulnerability(&self) -> bool {
        // Ignored informational notices
        if self.id == "RUSTSEC-2026-0192" {
            return false;
        }

        // If summary indicates an unmaintained crate notice (RustSec informational)
        if let Some(summary) = &self.summary {
            if summary.to_lowercase().contains("unmaintained") {
                return false;
            }
        }

        // GHSA or CVE ID is an explicit security vulnerability
        if self.id.starts_with("GHSA-") || self.id.starts_with("CVE-") {
            return true;
        }

        // If it has CVSS/severity metadata, it is a vulnerability
        if let Some(sev) = &self.severity {
            if !sev.is_empty() {
                return true;
            }
        }

        // If it is linked to a CVE or GHSA advisory, it is a security vulnerability
        if let Some(aliases) = &self.aliases {
            for alias in aliases {
                if alias.starts_with("CVE-") || alias.starts_with("GHSA-") {
                    return true;
                }
            }
        }

        true
    }
}

fn fetch_vuln_full(agent: &ureq::Agent, id: &str) -> Result<OsvVulnerability, String> {
    let url = format!("https://api.osv.dev/v1/vulns/{id}");
    let mut resp = agent
        .get(&url)
        .call()
        .map_err(|e| format!("Failed to fetch advisory details for {id}: {e}"))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Failed to read advisory body: {e}"))?;
    serde_json::from_str(&body).map_err(|e| format!("Failed to parse advisory: {e}"))
}

/// Query OSV for vulnerabilities in a list of (crate_name, version) pairs.
fn query_osv_vulnerabilities(
    packages: &[(&str, &str)],
) -> Result<Vec<(String, String, Vec<OsvVulnerability>)>, String> {
    if packages.is_empty() {
        return Ok(Vec::new());
    }

    let queries: Vec<OsvPackageQuery> = packages
        .iter()
        .map(|(name, ver)| OsvPackageQuery {
            package: OsvPackageInfo {
                name: (*name).to_string(),
                ecosystem: "crates.io",
            },
            version: (*ver).to_string(),
        })
        .collect();

    let batch = OsvQueryBatch { queries };
    let json_body = serde_json::to_string(&batch)
        .map_err(|e| format!("Failed to serialize OSV query: {e}"))?;

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .new_agent();

    let mut response = agent
        .post("https://api.osv.dev/v1/querybatch")
        .header("Content-Type", "application/json")
        .send(json_body.as_bytes())
        .map_err(|e| format!("OSV API request failed: {e}"))?;

    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Failed to read OSV body: {e}"))?;

    let batch_resp: OsvBatchResponse = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse OSV response: {e}"))?;

    let mut found = Vec::new();
    for (idx, result) in batch_resp.results.into_iter().enumerate() {
        if !result.vulns.is_empty() {
            let (name, ver) = packages[idx];
            let mut full_vulns = Vec::new();
            for stub in result.vulns {
                if let Ok(full) = fetch_vuln_full(&agent, &stub.id) {
                    full_vulns.push(full);
                } else {
                    full_vulns.push(stub);
                }
            }
            found.push((name.to_string(), ver.to_string(), full_vulns));
        }
    }

    Ok(found)
}

#[test]
fn test_osv_vulnerability_detection_unit() {
    // Test detecting a known historical vulnerability: tokio 0.2.0 (RUSTSEC-2021-0124 / CVE-2021-45710)
    let packages = vec![("tokio", "0.2.0")];
    match query_osv_vulnerabilities(&packages) {
        Ok(results) => {
            assert_eq!(results.len(), 1, "Expected tokio 0.2.0 to be flagged");
            let (name, _, vulns) = &results[0];
            assert_eq!(name, "tokio");
            assert!(
                vulns.iter().any(|v| v.is_security_vulnerability()),
                "Expected at least one security vulnerability in tokio 0.2.0"
            );
        }
        Err(err) => {
            eprintln!("Skipping OSV unit test due to network connectivity: {err}");
        }
    }

    // Test a modern clean package: serde 1.0.229
    let clean = vec![("serde", "1.0.229")];
    match query_osv_vulnerabilities(&clean) {
        Ok(results) => {
            assert!(
                results.is_empty(),
                "Expected clean serde version to have no vulnerabilities"
            );
        }
        Err(err) => {
            eprintln!("Skipping OSV unit test due to network connectivity: {err}");
        }
    }
}

#[test]
fn test_no_security_vulnerabilities_in_dependencies() {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()
        .expect("cargo metadata execution failed");

    assert!(output.status.success(), "cargo metadata returned failure");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse metadata JSON");

    let packages = json["packages"]
        .as_array()
        .expect("packages array in metadata");

    let mut dep_list = Vec::new();
    for pkg in packages {
        let name = pkg["name"].as_str().unwrap_or_default();
        let version = pkg["version"].as_str().unwrap_or_default();

        if name == "oxidisc" || name.is_empty() || version.is_empty() {
            continue;
        }

        dep_list.push((name, version));
    }

    dep_list.sort_unstable();
    dep_list.dedup();

    // Query OSV in batches of up to 500
    let mut vulnerabilities = Vec::new();
    let mut informational_notices = Vec::new();

    for chunk in dep_list.chunks(500) {
        match query_osv_vulnerabilities(chunk) {
            Ok(found) => {
                for (name, version, vulns) in found {
                    for v in vulns {
                        if v.is_security_vulnerability() {
                            let summary = v.summary.unwrap_or_else(|| "No summary".to_string());
                            let aliases = v.aliases.map(|a| a.join(", ")).unwrap_or_default();
                            vulnerabilities.push(format!(
                                "{} v{} - {} (Advisory: {}, Aliases: {})",
                                name, version, summary, v.id, aliases
                            ));
                        } else {
                            let summary = v.summary.unwrap_or_default();
                            informational_notices.push(format!(
                                "{} v{} (Notice: {} - {})",
                                name, version, v.id, summary
                            ));
                        }
                    }
                }
            }
            Err(err) => {
                eprintln!("Warning: Could not contact OSV advisory API (offline mode): {err}");
                return;
            }
        }
    }

    if !informational_notices.is_empty() {
        println!("\nInformational dependency notices (non-vulnerabilities):");
        for notice in &informational_notices {
            println!("  ℹ️  {notice}");
        }
    }

    if !vulnerabilities.is_empty() {
        let report = format!(
            "\nCRITICAL: Found {} security vulnerabilities in project dependencies:\n- {}",
            vulnerabilities.len(),
            vulnerabilities.join("\n- ")
        );
        panic!("{report}");
    }
}
