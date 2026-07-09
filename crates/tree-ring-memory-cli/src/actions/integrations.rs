use std::path::PathBuf;

use crate::integrations::{scan_integrations, IntegrationScanReport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationScanRequest {
    pub source_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntegrationScanActionReport {
    pub report: IntegrationScanReport,
}

pub fn scan(request: IntegrationScanRequest) -> IntegrationScanActionReport {
    IntegrationScanActionReport {
        report: scan_integrations(&request.source_root),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn integration_action_scans_project_markers() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Rules").unwrap();

        let report = scan(IntegrationScanRequest {
            source_root: dir.path().to_path_buf(),
        });

        assert!(report.report.detected_count > 0);
    }
}
