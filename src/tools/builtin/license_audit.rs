//! License audit tool for Rust projects.
//!
//! Scans dependency tree for licenses, detects incompatible licenses,
//! generates compliance reports and NOTICE files.
//!
//! Actions: scan, report, check, notice, compatible

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(LicenseAuditTool));
}

struct LicenseAuditTool;

#[async_trait::async_trait]
impl Tool for LicenseAuditTool {
    fn name(&self) -> &str {
        "license_audit"
    }

    fn description(&self) -> &str {
        "License audit tool for Rust projects. Actions: scan (scan dependency licenses), \
         report (generate compliance report), check (check for incompatible licenses), \
         notice (generate NOTICE file), compatible (check license compatibility)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: scan, report, check, notice, compatible".to_string(),
                required: true,
            },
            ToolParameter {
                name: "path".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to the Rust project directory (default: current directory)"
                    .to_string(),
                required: false,
            },
            ToolParameter {
                name: "output".to_string(),
                parameter_type: "string".to_string(),
                description: "Output file path (for report/notice actions)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "deny".to_string(),
                parameter_type: "string".to_string(),
                description: "Comma-separated list of denied licenses (e.g. 'GPL-3.0,AGPL-3.0')"
                    .to_string(),
                required: false,
            },
            ToolParameter {
                name: "allow".to_string(),
                parameter_type: "string".to_string(),
                description:
                    "Comma-separated list of allowed licenses (e.g. 'MIT,Apache-2.0,BSD-3-Clause')"
                        .to_string(),
                required: false,
            },
            ToolParameter {
                name: "project_license".to_string(),
                parameter_type: "string".to_string(),
                description:
                    "Your project's license for compatibility check (e.g. 'MIT', 'Apache-2.0')"
                        .to_string(),
                required: false,
            },
            ToolParameter {
                name: "format".to_string(),
                parameter_type: "string".to_string(),
                description: "Output format: json, markdown, text (default: markdown)".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let project_path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        match action {
            "scan" => self.scan_licenses(project_path, params),
            "report" => self.generate_report(project_path, params),
            "check" => self.check_compliance(project_path, params),
            "notice" => self.generate_notice(project_path, params),
            "compatible" => self.check_compatibility(project_path, params),
            _ => Err(format!(
                "Unknown action: {action}. Valid: scan, report, check, notice, compatible"
            )),
        }
    }
}

impl LicenseAuditTool {
    fn scan_licenses(
        &self,
        project_path: &str,
        _params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let packages = self.parse_cargo_metadata(project_path)?;

        let mut deps_with_licenses: Vec<DepLicense> = Vec::new();
        let mut unknown_licenses: Vec<String> = Vec::new();

        for package in &packages {
            if let Some(license) = self.detect_license(package) {
                let risk = self.assess_risk(&license);
                deps_with_licenses.push(DepLicense {
                    name: package.name.clone(),
                    version: package.version.clone(),
                    license: license.clone(),
                    risk_level: risk,
                    is_copyleft: self.is_copyleft(&license),
                });
            } else {
                unknown_licenses.push(format!("{}@{}", package.name, package.version));
            }
        }

        deps_with_licenses.sort_by(|a, b| a.name.cmp(&b.name));

        let risk_summary = self.count_risk_levels(&deps_with_licenses);

        Ok(serde_json::json!({
            "action": "scan",
            "total_dependencies": packages.len(),
            "licensed": deps_with_licenses.len(),
            "unknown_license": unknown_licenses.len(),
            "dependencies": deps_with_licenses.iter().map(|d| d.to_json()).collect::<Vec<_>>(),
            "unknown_packages": unknown_licenses,
            "risk_summary": {
                "low": risk_summary.low,
                "medium": risk_summary.medium,
                "high": risk_summary.high,
                "critical": risk_summary.critical,
            },
        }))
    }

    fn generate_report(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let scan_result = self.scan_licenses(project_path, params)?;
        let format = params
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("markdown");

        let output = params.get("output").and_then(|v| v.as_str());

        let report_content = match format {
            "markdown" => self.format_markdown_report(&scan_result),
            "json" => serde_json::to_string_pretty(&scan_result)
                .map_err(|e| format!("Failed to serialize report: {e}"))?,
            "text" => self.format_text_report(&scan_result),
            _ => {
                return Err(format!(
                    "Unknown format: {format}. Valid: markdown, json, text"
                ))
            }
        };

        if let Some(output_path) = output {
            fs::write(output_path, &report_content)
                .map_err(|e| format!("Failed to write report: {e}"))?;
        }

        Ok(serde_json::json!({
            "action": "report",
            "format": format,
            "output": output.unwrap_or("(stdout)"),
            "preview": if report_content.len() > 500 {
                format!("{}...", &report_content[..500])
            } else {
                report_content.clone()
            },
        }))
    }

    fn check_compliance(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let scan_result = self.scan_licenses(project_path, params)?;
        let empty_array = serde_json::Value::Array(vec![]);
        let deps = scan_result["dependencies"].as_array().unwrap_or_else(|| {
            if let Value::Array(a) = &empty_array {
                return a;
            }
            unreachable!()
        });

        let deny_list: Vec<String> = params
            .get("deny")
            .and_then(|v| v.as_str())
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|| {
                vec![
                    "AGPL-3.0".to_string(),
                    "SSPL-1.0".to_string(),
                    "EUPL-1.1".to_string(),
                ]
            });

        let allow_list: Option<Vec<String>> = params
            .get("allow")
            .and_then(|v| v.as_str())
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect());

        let mut violations: Vec<Violation> = Vec::new();

        for dep in deps {
            let name = dep["name"].as_str().unwrap_or("");
            let version = dep["version"].as_str().unwrap_or("");
            let license = dep["license"].as_str().unwrap_or("");
            let risk = dep["risk_level"].as_str().unwrap_or("");

            // Check deny list
            if deny_list.iter().any(|d| license.contains(d.as_str())) {
                violations.push(Violation {
                    package: format!("{name}@{version}"),
                    license: license.to_string(),
                    reason: format!("License '{license}' is in the deny list"),
                    severity: "critical".to_string(),
                });
            }

            // Check allow list
            if let Some(ref allowed) = allow_list {
                if !allowed.iter().any(|a| license.contains(a.as_str())) {
                    violations.push(Violation {
                        package: format!("{name}@{version}"),
                        license: license.to_string(),
                        reason: format!("License '{license}' is not in the allowed list"),
                        severity: "warning".to_string(),
                    });
                }
            }

            // Check copyleft risk
            if risk == "high" || risk == "critical" {
                violations.push(Violation {
                    package: format!("{name}@{version}"),
                    license: license.to_string(),
                    reason: "Copyleft license may impose obligations on derivative works"
                        .to_string(),
                    severity: if risk == "critical" {
                        "critical"
                    } else {
                        "warning"
                    }
                    .to_string(),
                });
            }
        }

        let compliant = violations.is_empty();

        Ok(serde_json::json!({
            "action": "check",
            "compliant": compliant,
            "violations": violations.iter().map(|v| v.to_json()).collect::<Vec<_>>(),
            "violation_count": violations.len(),
            "critical_violations": violations.iter().filter(|v| v.severity == "critical").count(),
            "warning_violations": violations.iter().filter(|v| v.severity == "warning").count(),
            "deny_list": deny_list,
            "allow_list": allow_list,
        }))
    }

    fn generate_notice(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let scan_result = self.scan_licenses(project_path, params)?;
        let empty_array = vec![];
        let deps = scan_result["dependencies"]
            .as_array()
            .unwrap_or(&empty_array);

        let mut notice = String::new();
        notice.push_str("THIRD-PARTY SOFTWARE NOTICES AND INFORMATION\n");
        notice.push_str("================================================\n\n");
        notice.push_str("This project incorporates components from the projects listed below.\n");
        notice.push_str("The original copyright notices and license terms are provided.\n\n");

        // Group by license
        let mut by_license: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for dep in deps {
            let name = dep["name"].as_str().unwrap_or("");
            let version = dep["version"].as_str().unwrap_or("");
            let license = dep["license"].as_str().unwrap_or("Unknown");

            by_license
                .entry(license.to_string())
                .or_default()
                .push((name.to_string(), version.to_string()));
        }

        let mut licenses: Vec<_> = by_license.iter().collect();
        licenses.sort_by(|a, b| a.0.cmp(b.0));

        for (license, packages) in &licenses {
            notice.push_str(&format!("--- {} ---\n\n", license));
            for (name, version) in *packages {
                notice.push_str(&format!("  {} ({})\n", name, version));
            }
            notice.push('\n');

            // Add standard license text references
            notice.push_str(self.get_license_reference(license));
            notice.push_str("\n\n");
        }

        let output = params
            .get("output")
            .and_then(|v| v.as_str())
            .unwrap_or("NOTICE");

        fs::write(output, &notice).map_err(|e| format!("Failed to write NOTICE file: {e}"))?;

        Ok(serde_json::json!({
            "action": "notice",
            "output": output,
            "license_groups": licenses.len(),
            "total_dependencies": deps.len(),
        }))
    }

    fn check_compatibility(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let project_license = params
            .get("project_license")
            .and_then(|v| v.as_str())
            .unwrap_or("MIT");

        let scan_result = self.scan_licenses(project_path, params)?;
        let empty_array = vec![];
        let deps = scan_result["dependencies"]
            .as_array()
            .unwrap_or(&empty_array);

        let mut compatible: Vec<Value> = Vec::new();
        let mut incompatible: Vec<Value> = Vec::new();
        let mut warnings: Vec<Value> = Vec::new();

        for dep in deps {
            let name = dep["name"].as_str().unwrap_or("");
            let version = dep["version"].as_str().unwrap_or("");
            let license = dep["license"].as_str().unwrap_or("");

            match self.check_license_compat(project_license, license) {
                LicenseCompat::Compatible => {
                    compatible.push(serde_json::json!({
                        "package": format!("{name}@{version}"),
                        "license": license,
                    }));
                }
                LicenseCompat::Warning(msg) => {
                    warnings.push(serde_json::json!({
                        "package": format!("{name}@{version}"),
                        "license": license,
                        "warning": msg,
                    }));
                }
                LicenseCompat::Incompatible(msg) => {
                    incompatible.push(serde_json::json!({
                        "package": format!("{name}@{version}"),
                        "license": license,
                        "reason": msg,
                    }));
                }
            }
        }

        Ok(serde_json::json!({
            "action": "compatible",
            "project_license": project_license,
            "compatible_count": compatible.len(),
            "warning_count": warnings.len(),
            "incompatible_count": incompatible.len(),
            "compatible": compatible,
            "warnings": warnings,
            "incompatible": incompatible,
            "overall": if incompatible.is_empty() { "PASS" } else { "FAIL" },
        }))
    }

    fn parse_cargo_metadata(&self, project_path: &str) -> Result<Vec<Package>, String> {
        let full_output = std::process::Command::new("cargo")
            .args(["metadata", "--format-version", "1"])
            .current_dir(project_path)
            .output()
            .map_err(|e| format!("Failed to run cargo metadata: {e}"))?;

        if !full_output.status.success() {
            return Err(format!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&full_output.stderr)
            ));
        }

        let full_metadata = String::from_utf8_lossy(&full_output.stdout);
        let full_json: serde_json::Value = serde_json::from_str(&full_metadata)
            .map_err(|e| format!("Failed to parse cargo metadata: {e}"))?;

        let mut result = Vec::new();
        if let Some(packages) = full_json["packages"].as_array() {
            for pkg in packages {
                if let (Some(name), Some(version), Some(license)) = (
                    pkg["name"].as_str(),
                    pkg["version"].as_str(),
                    pkg["license"].as_str(),
                ) {
                    result.push(Package {
                        name: name.to_string(),
                        version: version.to_string(),
                        license: Some(license.to_string()),
                        license_file: pkg["license_file"].as_str().map(|s| s.to_string()),
                    });
                }
            }
        }

        if result.is_empty() {
            return Err("No packages found in cargo metadata".to_string());
        }

        Ok(result)
    }

    fn detect_license(&self, package: &Package) -> Option<String> {
        if let Some(ref license) = package.license {
            if !license.is_empty() {
                return Some(self.normalize_license(license));
            }
        }

        if package.license_file.is_some() {
            return Some("Unknown (has license file)".to_string());
        }

        None
    }

    fn normalize_license(&self, license: &str) -> String {
        if license.contains(" OR ") || license.contains(" AND ") {
            return license.to_string();
        }
        license.trim().to_string()
    }

    fn assess_risk(&self, license: &str) -> &'static str {
        if self.is_permissive(license) {
            "low"
        } else if self.is_weak_copyleft(license) {
            "medium"
        } else if self.is_strong_copyleft(license) {
            "high"
        } else if self.is_network_copyleft(license) {
            "critical"
        } else {
            "medium"
        }
    }

    fn is_copyleft(&self, license: &str) -> bool {
        self.is_weak_copyleft(license)
            || self.is_strong_copyleft(license)
            || self.is_network_copyleft(license)
    }

    fn is_permissive(&self, license: &str) -> bool {
        let lower = license.to_lowercase();
        lower.contains("mit")
            || lower.contains("apache-2.0")
            || lower.contains("apache 2.0")
            || lower.contains("bsd-2-clause")
            || lower.contains("bsd-3-clause")
            || lower.contains("bsd")
            || lower.contains("isc")
            || lower.contains("unlicense")
            || lower.contains("cc0")
            || lower.contains("zlib")
            || lower.contains("bsl-1.0")
    }

    fn is_weak_copyleft(&self, license: &str) -> bool {
        let lower = license.to_lowercase();
        lower.contains("mpl-2.0")
            || lower.contains("mozilla public license")
            || lower.contains("lgpl-2.1")
            || lower.contains("lgpl-3.0")
            || lower.contains("lgpl")
            || lower.contains("epl-1.0")
            || lower.contains("epl-2.0")
            || lower.contains("cddl-1.0")
    }

    fn is_strong_copyleft(&self, license: &str) -> bool {
        let lower = license.to_lowercase();
        lower.contains("gpl-2.0")
            || lower.contains("gpl-3.0")
            || lower.contains("gpl-2.0-only")
            || lower.contains("gpl-3.0-only")
            || lower.contains("gpl-2.0-or-later")
            || lower.contains("gpl-3.0-or-later")
            || lower.contains("agpl-3.0")
            || (lower.contains("gpl") && !lower.contains("lgpl"))
    }

    fn is_network_copyleft(&self, license: &str) -> bool {
        let lower = license.to_lowercase();
        lower.contains("agpl")
            || lower.contains("sspl")
            || lower.contains("ossl")
            || lower.contains("buse")
    }

    fn count_risk_levels(&self, deps: &[DepLicense]) -> RiskSummary {
        let mut summary = RiskSummary::default();
        for dep in deps {
            match dep.risk_level {
                "low" => summary.low += 1,
                "medium" => summary.medium += 1,
                "high" => summary.high += 1,
                "critical" => summary.critical += 1,
                _ => {}
            }
        }
        summary
    }

    fn format_markdown_report(&self, scan_result: &Value) -> String {
        let mut report = String::new();

        report.push_str("# License Compliance Report\n\n");
        report.push_str(&format!(
            "**Total Dependencies:** {}  \n",
            scan_result["total_dependencies"].as_u64().unwrap_or(0)
        ));
        report.push_str(&format!(
            "**Licensed:** {}  \n",
            scan_result["licensed"].as_u64().unwrap_or(0)
        ));
        report.push_str(&format!(
            "**Unknown:** {}  \n\n",
            scan_result["unknown_license"].as_u64().unwrap_or(0)
        ));

        let risk = &scan_result["risk_summary"];
        report.push_str("## Risk Summary\n\n");
        report.push_str("| Risk Level | Count |\n|------------|-------|\n");
        report.push_str(&format!(
            "| Low | {} |\n",
            risk["low"].as_u64().unwrap_or(0)
        ));
        report.push_str(&format!(
            "| Medium | {} |\n",
            risk["medium"].as_u64().unwrap_or(0)
        ));
        report.push_str(&format!(
            "| High | {} |\n",
            risk["high"].as_u64().unwrap_or(0)
        ));
        report.push_str(&format!(
            "| Critical | {} |\n\n",
            risk["critical"].as_u64().unwrap_or(0)
        ));

        if let Some(deps) = scan_result["dependencies"].as_array() {
            report.push_str("## Dependencies\n\n");
            report.push_str("| Package | Version | License | Risk | Copyleft |\n");
            report.push_str("|---------|---------|---------|------|----------|\n");

            for dep in deps {
                let name = dep["name"].as_str().unwrap_or("");
                let version = dep["version"].as_str().unwrap_or("");
                let license = dep["license"].as_str().unwrap_or("");
                let risk_level = dep["risk_level"].as_str().unwrap_or("");
                let copyleft = if dep["is_copyleft"].as_bool().unwrap_or(false) {
                    "Yes"
                } else {
                    "No"
                };

                report.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    name, version, license, risk_level, copyleft
                ));
            }
        }

        if let Some(unknown) = scan_result["unknown_packages"].as_array() {
            if !unknown.is_empty() {
                report.push_str("\n## Unknown Licenses\n\n");
                for pkg in unknown {
                    if let Some(name) = pkg.as_str() {
                        report.push_str(&format!("- {}\n", name));
                    }
                }
            }
        }

        report
    }

    fn format_text_report(&self, scan_result: &Value) -> String {
        let mut report = String::new();

        report.push_str("LICENSE COMPLIANCE REPORT\n");
        report.push_str("========================\n\n");
        report.push_str(&format!(
            "Total Dependencies: {}\n",
            scan_result["total_dependencies"].as_u64().unwrap_or(0)
        ));

        let risk = &scan_result["risk_summary"];
        report.push_str(&format!(
            "Low Risk: {}\n",
            risk["low"].as_u64().unwrap_or(0)
        ));
        report.push_str(&format!(
            "Medium Risk: {}\n",
            risk["medium"].as_u64().unwrap_or(0)
        ));
        report.push_str(&format!(
            "High Risk: {}\n",
            risk["high"].as_u64().unwrap_or(0)
        ));
        report.push_str(&format!(
            "Critical Risk: {}\n\n",
            risk["critical"].as_u64().unwrap_or(0)
        ));

        if let Some(deps) = scan_result["dependencies"].as_array() {
            for dep in deps {
                let name = dep["name"].as_str().unwrap_or("");
                let version = dep["version"].as_str().unwrap_or("");
                let license = dep["license"].as_str().unwrap_or("");
                let risk_level = dep["risk_level"].as_str().unwrap_or("");

                report.push_str(&format!(
                    "{}@{}  [{}] ({})\n",
                    name, version, license, risk_level
                ));
            }
        }

        report
    }

    fn check_license_compat(&self, project: &str, dependency: &str) -> LicenseCompat {
        let proj_lower = project.to_lowercase();
        let dep_lower = dependency.to_lowercase();

        if proj_lower.contains("mit") {
            if self.is_permissive(&dep_lower) {
                return LicenseCompat::Compatible;
            }
            if self.is_weak_copyleft(&dep_lower) {
                return LicenseCompat::Warning(
                    "Weak copyleft license - may require source disclosure for modifications"
                        .to_string(),
                );
            }
            if self.is_strong_copyleft(&dep_lower) {
                return LicenseCompat::Incompatible(
                    "Strong copyleft license (GPL) is incompatible with MIT distribution"
                        .to_string(),
                );
            }
            if self.is_network_copyleft(&dep_lower) {
                return LicenseCompat::Incompatible(
                    "Network copyleft license (AGPL/SSPL) is incompatible with MIT distribution"
                        .to_string(),
                );
            }
        }

        if proj_lower.contains("apache-2.0") || proj_lower.contains("apache 2.0") {
            if self.is_permissive(&dep_lower) || dep_lower.contains("apache-2.0") {
                return LicenseCompat::Compatible;
            }
            if dep_lower.contains("gpl-2.0") {
                return LicenseCompat::Incompatible(
                    "GPL-2.0 is incompatible with Apache-2.0 due to patent clause".to_string(),
                );
            }
            if self.is_weak_copyleft(&dep_lower) {
                return LicenseCompat::Warning(
                    "Weak copyleft license may impose obligations".to_string(),
                );
            }
            if self.is_strong_copyleft(&dep_lower) || self.is_network_copyleft(&dep_lower) {
                return LicenseCompat::Incompatible(
                    "Strong/network copyleft license is incompatible with Apache-2.0".to_string(),
                );
            }
        }

        if proj_lower.contains("gpl-3.0") {
            if self.is_permissive(&dep_lower)
                || self.is_weak_copyleft(&dep_lower)
                || self.is_strong_copyleft(&dep_lower)
            {
                return LicenseCompat::Compatible;
            }
            if self.is_network_copyleft(&dep_lower) {
                return LicenseCompat::Warning(
                    "AGPL/SSPL has additional network use requirements".to_string(),
                );
            }
        }

        LicenseCompat::Compatible
    }

    fn get_license_reference(&self, license: &str) -> &'static str {
        let lower = license.to_lowercase();
        if lower.contains("mit") {
            "  Licensed under the MIT License.\n  See https://opensource.org/licenses/MIT"
        } else if lower.contains("apache-2.0") || lower.contains("apache 2.0") {
            "  Licensed under the Apache License, Version 2.0.\n  See https://www.apache.org/licenses/LICENSE-2.0"
        } else if lower.contains("bsd-3-clause") || lower.contains("bsd 3") {
            "  Licensed under the BSD 3-Clause License.\n  See https://opensource.org/licenses/BSD-3-Clause"
        } else if lower.contains("bsd-2-clause") || lower.contains("bsd 2") {
            "  Licensed under the BSD 2-Clause License.\n  See https://opensource.org/licenses/BSD-2-Clause"
        } else if lower.contains("mpl-2.0") || lower.contains("mozilla") {
            "  Licensed under the Mozilla Public License 2.0.\n  See https://www.mozilla.org/en-US/MPL/2.0/"
        } else if lower.contains("gpl-3.0") {
            "  Licensed under the GNU General Public License v3.0.\n  See https://www.gnu.org/licenses/gpl-3.0"
        } else if lower.contains("gpl-2.0") {
            "  Licensed under the GNU General Public License v2.0.\n  See https://www.gnu.org/licenses/old-licenses/gpl-2.0"
        } else if lower.contains("lgpl") {
            "  Licensed under the GNU Lesser General Public License.\n  See https://www.gnu.org/licenses/lgpl-3.0"
        } else if lower.contains("isc") {
            "  Licensed under the ISC License.\n  See https://opensource.org/licenses/ISC"
        } else if lower.contains("unlicense") {
            "  This work is released into the public domain (Unlicense).\n  See https://unlicense.org/"
        } else if lower.contains("bsl-1.0") {
            "  Licensed under the Boost Software License 1.0.\n  See https://www.boost.org/LICENSE_1_0.txt"
        } else {
            "  License terms as specified by the copyright holder."
        }
    }
}

struct Package {
    name: String,
    version: String,
    license: Option<String>,
    license_file: Option<String>,
}

struct DepLicense {
    name: String,
    version: String,
    license: String,
    risk_level: &'static str,
    is_copyleft: bool,
}

impl DepLicense {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "version": self.version,
            "license": self.license,
            "risk_level": self.risk_level,
            "is_copyleft": self.is_copyleft,
        })
    }
}

#[derive(Default)]
struct RiskSummary {
    low: usize,
    medium: usize,
    high: usize,
    critical: usize,
}


struct Violation {
    package: String,
    license: String,
    reason: String,
    severity: String,
}

impl Violation {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "package": self.package,
            "license": self.license,
            "reason": self.reason,
            "severity": self.severity,
        })
    }
}

enum LicenseCompat {
    Compatible,
    Warning(String),
    Incompatible(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool() -> LicenseAuditTool {
        LicenseAuditTool
    }

    // ===== License classification tests =====

    #[test]
    fn test_is_permissive_mit() {
        let tool = make_tool();
        assert!(tool.is_permissive("MIT"));
        assert!(tool.is_permissive("mit"));
    }

    #[test]
    fn test_is_permissive_apache() {
        let tool = make_tool();
        assert!(tool.is_permissive("Apache-2.0"));
        assert!(tool.is_permissive("Apache 2.0"));
    }

    #[test]
    fn test_is_permissive_bsd() {
        let tool = make_tool();
        assert!(tool.is_permissive("BSD-2-Clause"));
        assert!(tool.is_permissive("BSD-3-Clause"));
        assert!(tool.is_permissive("BSD"));
    }

    #[test]
    fn test_is_permissive_misc() {
        let tool = make_tool();
        assert!(tool.is_permissive("ISC"));
        assert!(tool.is_permissive("Unlicense"));
        assert!(tool.is_permissive("CC0-1.0"));
        assert!(tool.is_permissive("Zlib"));
        assert!(tool.is_permissive("BSL-1.0"));
    }

    #[test]
    fn test_is_permissive_gpl_false() {
        let tool = make_tool();
        assert!(!tool.is_permissive("GPL-3.0"));
        assert!(!tool.is_permissive("LGPL-2.1"));
        assert!(!tool.is_permissive("AGPL-3.0"));
    }

    #[test]
    fn test_is_weak_copyleft() {
        let tool = make_tool();
        assert!(tool.is_weak_copyleft("MPL-2.0"));
        assert!(tool.is_weak_copyleft("Mozilla Public License 2.0"));
        assert!(tool.is_weak_copyleft("LGPL-2.1"));
        assert!(tool.is_weak_copyleft("LGPL-3.0"));
        assert!(tool.is_weak_copyleft("LGPL"));
        assert!(tool.is_weak_copyleft("EPL-1.0"));
        assert!(tool.is_weak_copyleft("EPL-2.0"));
        assert!(tool.is_weak_copyleft("CDDL-1.0"));
    }

    #[test]
    fn test_is_weak_copyleft_mit_false() {
        let tool = make_tool();
        assert!(!tool.is_weak_copyleft("MIT"));
        assert!(!tool.is_weak_copyleft("Apache-2.0"));
    }

    #[test]
    fn test_is_strong_copyleft() {
        let tool = make_tool();
        assert!(tool.is_strong_copyleft("GPL-2.0"));
        assert!(tool.is_strong_copyleft("GPL-3.0"));
        assert!(tool.is_strong_copyleft("GPL-2.0-only"));
        assert!(tool.is_strong_copyleft("GPL-3.0-only"));
        assert!(tool.is_strong_copyleft("GPL-2.0-or-later"));
        assert!(tool.is_strong_copyleft("GPL-3.0-or-later"));
    }

    #[test]
    fn test_is_strong_copyleft_lgpl_false() {
        let tool = make_tool();
        // LGPL should NOT be strong copyleft (it's weak)
        assert!(!tool.is_strong_copyleft("LGPL-2.1"));
        assert!(!tool.is_strong_copyleft("LGPL-3.0"));
    }

    #[test]
    fn test_is_strong_copyleft_mit_false() {
        let tool = make_tool();
        assert!(!tool.is_strong_copyleft("MIT"));
        assert!(!tool.is_strong_copyleft("Apache-2.0"));
    }

    #[test]
    fn test_is_network_copyleft() {
        let tool = make_tool();
        assert!(tool.is_network_copyleft("AGPL-3.0"));
        assert!(tool.is_network_copyleft("SSPL-1.0"));
        assert!(!tool.is_network_copyleft("GPL-3.0"));
        assert!(!tool.is_network_copyleft("MIT"));
    }

    #[test]
    fn test_is_copyleft() {
        let tool = make_tool();
        assert!(tool.is_copyleft("GPL-3.0"));
        assert!(tool.is_copyleft("LGPL-2.1"));
        assert!(tool.is_copyleft("AGPL-3.0"));
        assert!(tool.is_copyleft("MPL-2.0"));
        assert!(!tool.is_copyleft("MIT"));
        assert!(!tool.is_copyleft("Apache-2.0"));
        assert!(!tool.is_copyleft("BSD-3-Clause"));
    }

    // ===== Risk assessment tests =====

    #[test]
    fn test_assess_risk_low() {
        let tool = make_tool();
        assert_eq!(tool.assess_risk("MIT"), "low");
        assert_eq!(tool.assess_risk("Apache-2.0"), "low");
        assert_eq!(tool.assess_risk("BSD-3-Clause"), "low");
        assert_eq!(tool.assess_risk("ISC"), "low");
    }

    #[test]
    fn test_assess_risk_medium() {
        let tool = make_tool();
        assert_eq!(tool.assess_risk("MPL-2.0"), "medium");
        assert_eq!(tool.assess_risk("LGPL-2.1"), "medium");
        assert_eq!(tool.assess_risk("EPL-2.0"), "medium");
    }

    #[test]
    fn test_assess_risk_high() {
        let tool = make_tool();
        assert_eq!(tool.assess_risk("GPL-3.0"), "high");
        assert_eq!(tool.assess_risk("GPL-2.0"), "high");
    }

    #[test]
    fn test_assess_risk_critical() {
        let tool = make_tool();
        assert_eq!(tool.assess_risk("AGPL-3.0"), "critical");
        assert_eq!(tool.assess_risk("SSPL-1.0"), "critical");
    }

    // ===== License normalization =====

    #[test]
    fn test_normalize_license_or() {
        let tool = make_tool();
        assert_eq!(tool.normalize_license("MIT OR Apache-2.0"), "MIT OR Apache-2.0");
    }

    #[test]
    fn test_normalize_license_and() {
        let tool = make_tool();
        assert_eq!(tool.normalize_license("MIT AND Apache-2.0"), "MIT AND Apache-2.0");
    }

    #[test]
    fn test_normalize_license_simple() {
        let tool = make_tool();
        assert_eq!(tool.normalize_license("  MIT  "), "MIT");
    }

    // ===== License compatibility tests =====

    #[test]
    fn test_compat_mit_with_permissive() {
        let tool = make_tool();
        assert!(matches!(
            tool.check_license_compat("MIT", "Apache-2.0"),
            LicenseCompat::Compatible
        ));
        assert!(matches!(
            tool.check_license_compat("MIT", "BSD-3-Clause"),
            LicenseCompat::Compatible
        ));
        assert!(matches!(
            tool.check_license_compat("MIT", "ISC"),
            LicenseCompat::Compatible
        ));
    }

    #[test]
    fn test_compat_mit_with_weak_copyleft() {
        let tool = make_tool();
        assert!(matches!(
            tool.check_license_compat("MIT", "MPL-2.0"),
            LicenseCompat::Warning(_)
        ));
    }

    #[test]
    fn test_compat_mit_with_strong_copyleft() {
        let tool = make_tool();
        assert!(matches!(
            tool.check_license_compat("MIT", "GPL-3.0"),
            LicenseCompat::Incompatible(_)
        ));
    }

    #[test]
    fn test_compat_mit_with_network_copyleft() {
        let tool = make_tool();
        assert!(matches!(
            tool.check_license_compat("MIT", "AGPL-3.0"),
            LicenseCompat::Incompatible(_)
        ));
    }

    #[test]
    fn test_compat_apache2_with_gpl2() {
        let tool = make_tool();
        assert!(matches!(
            tool.check_license_compat("Apache-2.0", "GPL-2.0"),
            LicenseCompat::Incompatible(_)
        ));
    }

    #[test]
    fn test_compat_apache2_with_permissive() {
        let tool = make_tool();
        assert!(matches!(
            tool.check_license_compat("Apache-2.0", "MIT"),
            LicenseCompat::Compatible
        ));
    }

    #[test]
    fn test_compat_gpl3_accepts_most() {
        let tool = make_tool();
        assert!(matches!(
            tool.check_license_compat("GPL-3.0", "MIT"),
            LicenseCompat::Compatible
        ));
        assert!(matches!(
            tool.check_license_compat("GPL-3.0", "Apache-2.0"),
            LicenseCompat::Compatible
        ));
        assert!(matches!(
            tool.check_license_compat("GPL-3.0", "LGPL-2.1"),
            LicenseCompat::Compatible
        ));
        assert!(matches!(
            tool.check_license_compat("GPL-3.0", "GPL-3.0"),
            LicenseCompat::Compatible
        ));
    }

    #[test]
    fn test_compat_gpl3_with_agpl() {
        let tool = make_tool();
        assert!(matches!(
            tool.check_license_compat("GPL-3.0", "AGPL-3.0"),
            LicenseCompat::Warning(_)
        ));
    }

    // ===== count_risk_levels =====

    #[test]
    fn test_count_risk_levels() {
        let tool = make_tool();
        let deps = vec![
            DepLicense { name: "a".into(), version: "1.0".into(), license: "MIT".into(), risk_level: "low", is_copyleft: false },
            DepLicense { name: "b".into(), version: "1.0".into(), license: "MPL-2.0".into(), risk_level: "medium", is_copyleft: true },
            DepLicense { name: "c".into(), version: "1.0".into(), license: "GPL-3.0".into(), risk_level: "high", is_copyleft: true },
            DepLicense { name: "d".into(), version: "1.0".into(), license: "AGPL-3.0".into(), risk_level: "critical", is_copyleft: true },
            DepLicense { name: "e".into(), version: "1.0".into(), license: "BSD".into(), risk_level: "low", is_copyleft: false },
        ];
        let summary = tool.count_risk_levels(&deps);
        assert_eq!(summary.low, 2);
        assert_eq!(summary.medium, 1);
        assert_eq!(summary.high, 1);
        assert_eq!(summary.critical, 1);
    }

    #[test]
    fn test_count_risk_levels_empty() {
        let tool = make_tool();
        let summary = tool.count_risk_levels(&[]);
        assert_eq!(summary.low, 0);
        assert_eq!(summary.medium, 0);
        assert_eq!(summary.high, 0);
        assert_eq!(summary.critical, 0);
    }

    // ===== DepLicense::to_json =====

    #[test]
    fn test_dep_license_to_json() {
        let dep = DepLicense {
            name: "serde".into(),
            version: "1.0".into(),
            license: "MIT".into(),
            risk_level: "low",
            is_copyleft: false,
        };
        let json = dep.to_json();
        assert_eq!(json["name"], "serde");
        assert_eq!(json["version"], "1.0");
        assert_eq!(json["license"], "MIT");
        assert_eq!(json["risk_level"], "low");
        assert_eq!(json["is_copyleft"], false);
    }

    // ===== Violation::to_json =====

    #[test]
    fn test_violation_to_json() {
        let v = Violation {
            package: "gpl-lib@1.0".into(),
            license: "GPL-3.0".into(),
            reason: "Denied license".into(),
            severity: "critical".into(),
        };
        let json = v.to_json();
        assert_eq!(json["package"], "gpl-lib@1.0");
        assert_eq!(json["license"], "GPL-3.0");
        assert_eq!(json["reason"], "Denied license");
        assert_eq!(json["severity"], "critical");
    }

    // ===== get_license_reference =====

    #[test]
    fn test_license_reference_mit() {
        let tool = make_tool();
        let ref_text = tool.get_license_reference("MIT");
        assert!(ref_text.contains("MIT License"));
        assert!(ref_text.contains("opensource.org/licenses/MIT"));
    }

    #[test]
    fn test_license_reference_apache() {
        let tool = make_tool();
        let ref_text = tool.get_license_reference("Apache-2.0");
        assert!(ref_text.contains("Apache License"));
        assert!(ref_text.contains("apache.org/licenses/LICENSE-2.0"));
    }

    #[test]
    fn test_license_reference_unknown() {
        let tool = make_tool();
        let ref_text = tool.get_license_reference("Custom-License");
        assert!(ref_text.contains("copyright holder"));
    }

    // ===== normalize_license =====

    #[test]
    fn test_detect_license_from_field() {
        let tool = make_tool();
        let pkg = Package {
            name: "test".into(),
            version: "1.0".into(),
            license: Some("MIT".into()),
            license_file: None,
        };
        assert_eq!(tool.detect_license(&pkg), Some("MIT".to_string()));
    }

    #[test]
    fn test_detect_license_from_file() {
        let tool = make_tool();
        let pkg = Package {
            name: "test".into(),
            version: "1.0".into(),
            license: None,
            license_file: Some("LICENSE".into()),
        };
        assert_eq!(
            tool.detect_license(&pkg),
            Some("Unknown (has license file)".to_string())
        );
    }

    #[test]
    fn test_detect_license_none() {
        let tool = make_tool();
        let pkg = Package {
            name: "test".into(),
            version: "1.0".into(),
            license: None,
            license_file: None,
        };
        assert_eq!(tool.detect_license(&pkg), None);
    }

    #[test]
    fn test_detect_license_empty_string() {
        let tool = make_tool();
        let pkg = Package {
            name: "test".into(),
            version: "1.0".into(),
            license: Some("".into()),
            license_file: None,
        };
        assert_eq!(tool.detect_license(&pkg), None);
    }

    // ===== format_text_report =====

    #[test]
    fn test_format_text_report_basic() {
        let tool = make_tool();
        let scan_result = serde_json::json!({
            "total_dependencies": 5,
            "risk_summary": {
                "low": 3,
                "medium": 1,
                "high": 1,
                "critical": 0,
            },
            "dependencies": [
                {"name": "serde", "version": "1.0", "license": "MIT", "risk_level": "low", "is_copyleft": false}
            ],
            "unknown_packages": [],
            "licensed": 1,
            "unknown_license": 0,
        });
        let report = tool.format_text_report(&scan_result);
        assert!(report.contains("LICENSE COMPLIANCE REPORT"));
        assert!(report.contains("Total Dependencies: 5"));
        assert!(report.contains("Low Risk: 3"));
        assert!(report.contains("High Risk: 1"));
    }

    // ===== format_markdown_report =====

    #[test]
    fn test_format_markdown_report_basic() {
        let tool = make_tool();
        let scan_result = serde_json::json!({
            "total_dependencies": 2,
            "licensed": 2,
            "unknown_license": 0,
            "risk_summary": { "low": 1, "medium": 1, "high": 0, "critical": 0 },
            "dependencies": [
                {"name": "serde", "version": "1.0", "license": "MIT", "risk_level": "low", "is_copyleft": false},
                {"name": "mpl-lib", "version": "2.0", "license": "MPL-2.0", "risk_level": "medium", "is_copyleft": true},
            ],
            "unknown_packages": [],
        });
        let report = tool.format_markdown_report(&scan_result);
        assert!(report.contains("# License Compliance Report"));
        assert!(report.contains("## Risk Summary"));
        assert!(report.contains("## Dependencies"));
        assert!(report.contains("serde"));
        assert!(report.contains("mpl-lib"));
    }
}
