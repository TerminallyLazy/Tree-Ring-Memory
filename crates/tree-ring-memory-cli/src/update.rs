use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RELEASE_API_URL: &str =
    "https://api.github.com/repos/TerminallyLazy/Tree-Ring-Memory/releases/latest";
const RELEASE_DOWNLOAD_PREFIX: &str =
    "https://github.com/TerminallyLazy/Tree-Ring-Memory/releases/download/";
const HOMEBREW_FORMULA: &str = "terminallylazy/tree-ring/tree-ring";

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InstallMethod {
    Homebrew,
    ProjectLocal { prefix: PathBuf },
    Direct { prefix: PathBuf },
}

impl InstallMethod {
    fn name(&self) -> &'static str {
        match self {
            Self::Homebrew => "homebrew",
            Self::ProjectLocal { .. } => "project-local",
            Self::Direct { .. } => "direct",
        }
    }
}

#[derive(Debug, Serialize)]
struct UpdateReport {
    current_version: String,
    latest_version: String,
    update_available: bool,
    updated: bool,
    install_method: String,
    executable: String,
    next_step: String,
}

pub(crate) fn run(check_only: bool, json_output: bool) -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to resolve the current tree-ring executable: {error}"))?;
    let executable = fs::canonicalize(&executable).unwrap_or(executable);
    let install_method = classify_install(&executable)?;
    let release = fetch_latest_release()?;
    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|error| format!("invalid installed Tree Ring version: {error}"))?;
    let latest = parse_release_version(&release.tag_name)?;
    let update_available = latest > current;

    let updated = if check_only || !update_available {
        false
    } else {
        apply_update(&release, &latest, &executable, &install_method)?;
        true
    };

    let next_step = if updated {
        "From each project root, run `tree-ring --root .tree-ring init` to refresh managed agent guidance."
            .to_string()
    } else if update_available {
        "Run `tree-ring update` to install this release in the same location.".to_string()
    } else {
        "Tree Ring Memory is up to date.".to_string()
    };
    let report = UpdateReport {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        update_available,
        updated,
        install_method: install_method.name().to_string(),
        executable: executable.display().to_string(),
        next_step,
    };
    print_report(&report, check_only, json_output)
}

fn print_report(report: &UpdateReport, check_only: bool, json_output: bool) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string(report).map_err(|error| error.to_string())?
        );
        return Ok(());
    }

    if report.updated {
        println!(
            "Updated Tree Ring Memory {} -> {} ({})",
            report.current_version, report.latest_version, report.install_method
        );
    } else if report.update_available {
        println!(
            "Tree Ring Memory {} is available (installed: {}, method: {}).",
            report.latest_version, report.current_version, report.install_method
        );
        if check_only {
            println!("No files were changed (--check).");
        }
    } else {
        println!("Tree Ring Memory {} is up to date.", report.current_version);
    }
    println!("{}", report.next_step);
    Ok(())
}

fn fetch_latest_release() -> Result<GithubRelease, String> {
    let body = download_text(RELEASE_API_URL)?;
    serde_json::from_str(&body)
        .map_err(|error| format!("failed to parse the latest Tree Ring release: {error}"))
}

fn download_text(url: &str) -> Result<String, String> {
    let output = if command_exists("curl") {
        Command::new("curl")
            .args([
                "-fsSL",
                "-H",
                "Accept: application/vnd.github+json",
                "-H",
                "X-GitHub-Api-Version: 2022-11-28",
                "-A",
                "tree-ring-memory-updater",
                url,
            ])
            .output()
    } else if command_exists("wget") {
        Command::new("wget")
            .args(["-qO-", "--user-agent=tree-ring-memory-updater", url])
            .output()
    } else {
        return Err("tree-ring update requires curl or wget".to_string());
    }
    .map_err(|error| format!("failed to start release download: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "failed to download {url}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| format!("downloaded response from {url} was not UTF-8"))
}

fn download_file(url: &str, destination: &Path) -> Result<(), String> {
    let status = if command_exists("curl") {
        Command::new("curl")
            .args(["-fsSL", "-A", "tree-ring-memory-updater", "-o"])
            .arg(destination)
            .arg(url)
            .status()
    } else if command_exists("wget") {
        Command::new("wget")
            .args(["-q", "--user-agent=tree-ring-memory-updater", "-O"])
            .arg(destination)
            .arg(url)
            .status()
    } else {
        return Err("tree-ring update requires curl or wget".to_string());
    }
    .map_err(|error| format!("failed to start release download: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("failed to download {url}"))
    }
}

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn parse_release_version(tag: &str) -> Result<Version, String> {
    Version::parse(tag.trim_start_matches('v'))
        .map_err(|error| format!("latest release has an invalid version `{tag}`: {error}"))
}

fn classify_install(executable: &Path) -> Result<InstallMethod, String> {
    let components = executable
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>();
    if components
        .windows(2)
        .any(|pair| pair[0] == "Cellar" && pair[1] == "tree-ring")
    {
        return Ok(InstallMethod::Homebrew);
    }

    let bin_dir = executable.parent().ok_or_else(|| {
        format!(
            "cannot determine the install prefix for {}",
            executable.display()
        )
    })?;
    if bin_dir.file_name().and_then(|name| name.to_str()) != Some("bin") {
        return Err(format!(
            "refusing to update an executable outside a bin directory: {}",
            executable.display()
        ));
    }
    let prefix = bin_dir.parent().ok_or_else(|| {
        format!(
            "cannot determine the install prefix for {}",
            executable.display()
        )
    })?;
    if prefix.file_name().and_then(|name| name.to_str()) == Some(".tree-ring") {
        Ok(InstallMethod::ProjectLocal {
            prefix: prefix.to_path_buf(),
        })
    } else {
        Ok(InstallMethod::Direct {
            prefix: prefix.to_path_buf(),
        })
    }
}

fn apply_update(
    release: &GithubRelease,
    latest: &Version,
    executable: &Path,
    install_method: &InstallMethod,
) -> Result<bool, String> {
    match install_method {
        InstallMethod::Homebrew => update_homebrew(latest),
        InstallMethod::ProjectLocal { prefix } | InstallMethod::Direct { prefix } => {
            update_direct(release, latest, executable, prefix)
        }
    }
}

fn update_homebrew(expected: &Version) -> Result<bool, String> {
    if !command_exists("brew") {
        return Err(
            "this Tree Ring binary is Homebrew-managed, but brew was not found".to_string(),
        );
    }
    let status = Command::new("brew")
        .args(["upgrade", HOMEBREW_FORMULA])
        .status()
        .map_err(|error| format!("failed to start Homebrew: {error}"))?;
    if !status.success() {
        return Err(format!(
            "Homebrew could not upgrade {HOMEBREW_FORMULA}; try `brew update` and rerun `tree-ring update`"
        ));
    }
    let prefix_output = Command::new("brew")
        .args(["--prefix", HOMEBREW_FORMULA])
        .output()
        .map_err(|error| format!("failed to resolve the Homebrew prefix: {error}"))?;
    if !prefix_output.status.success() {
        return Err(
            "Homebrew upgraded Tree Ring but did not report its install prefix".to_string(),
        );
    }
    let prefix = String::from_utf8(prefix_output.stdout)
        .map_err(|_| "Homebrew returned a non-UTF-8 install prefix".to_string())?;
    let installed = installed_version_from_path(Path::new(prefix.trim()).join("bin/tree-ring"))?;
    if &installed < expected {
        return Err(format!(
            "Homebrew installed {installed}, but the latest release is {expected}; the formula may still be updating"
        ));
    }
    Ok(true)
}

fn update_direct(
    release: &GithubRelease,
    latest: &Version,
    executable: &Path,
    prefix: &Path,
) -> Result<bool, String> {
    let platform = release_platform()?;
    let archive_name = format!("tree-ring-memory-{latest}-{platform}.tar.gz");
    let checksum_name = format!("{archive_name}.sha256");
    let archive_url = asset_url(release, &archive_name)?;
    let checksum_url = asset_url(release, &checksum_name)?;

    let temp = tempfile::Builder::new()
        .prefix("tree-ring-update-")
        .tempdir_in(prefix)
        .map_err(|error| {
            format!(
                "cannot create an update staging directory in {}: {error}",
                prefix.display()
            )
        })?;
    let archive = temp.path().join(&archive_name);
    download_file(archive_url, &archive)?;
    let expected_checksum = checksum_from_text(&download_text(checksum_url)?)?;
    verify_checksum(&archive, &expected_checksum)?;

    let unpacked = temp.path().join("unpacked");
    fs::create_dir(&unpacked).map_err(|error| error.to_string())?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(&archive)
        .arg("-C")
        .arg(&unpacked)
        .status()
        .map_err(|error| format!("failed to start tar: {error}"))?;
    if !status.success() {
        return Err("failed to unpack the verified Tree Ring release".to_string());
    }
    let released_binary = find_released_binary(&unpacked)?;
    let staged = temp.path().join("tree-ring.new");
    fs::copy(&released_binary, &staged)
        .map_err(|error| format!("failed to stage the updated binary: {error}"))?;
    make_executable(&staged)?;

    let staged_version = installed_version_from_path(&staged)?;
    if &staged_version != latest {
        return Err(format!(
            "verified release contained Tree Ring {staged_version}, expected {latest}"
        ));
    }
    fs::rename(&staged, executable)
        .map_err(|error| format!("failed to replace {}: {error}", executable.display()))?;
    Ok(true)
}

fn release_platform() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("darwin-arm64"),
        ("linux", "x86_64") => Ok("linux-x86_64"),
        (os, arch) => Err(format!(
            "no prebuilt Tree Ring release is available for {os}/{arch}"
        )),
    }
}

fn asset_url<'a>(release: &'a GithubRelease, name: &str) -> Result<&'a str, String> {
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .ok_or_else(|| format!("latest release does not include {name}"))?;
    if !asset
        .browser_download_url
        .starts_with(RELEASE_DOWNLOAD_PREFIX)
    {
        return Err(format!(
            "release asset {name} has an unexpected download URL"
        ));
    }
    Ok(&asset.browser_download_url)
}

fn checksum_from_text(text: &str) -> Result<String, String> {
    let checksum = text.split_whitespace().next().unwrap_or_default();
    if checksum.len() != 64
        || !checksum
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("release checksum file did not contain a valid SHA-256".to_string());
    }
    Ok(checksum.to_ascii_lowercase())
}

fn verify_checksum(path: &Path, expected: &str) -> Result<(), String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read release archive: {error}"))?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual == expected {
        Ok(())
    } else {
        Err("release archive checksum mismatch".to_string())
    }
}

fn find_released_binary(root: &Path) -> Result<PathBuf, String> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("failed to inspect release archive: {error}"))?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file() && entry.file_name() == "tree-ring" {
                return Ok(path);
            }
        }
    }
    Err("release archive did not contain tree-ring".to_string())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .map_err(|error| error.to_string())?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn installed_version_from_path(command: impl AsRef<Path>) -> Result<Version, String> {
    let command = command.as_ref();
    let output = Command::new(command)
        .arg("--version")
        .output()
        .map_err(|error| format!("failed to run {} --version: {error}", command.display()))?;
    parse_version_output(&output.stdout, output.status.success())
}

fn parse_version_output(stdout: &[u8], success: bool) -> Result<Version, String> {
    if !success {
        return Err("updated tree-ring binary did not run successfully".to_string());
    }
    let output = String::from_utf8_lossy(stdout);
    let version = output
        .split_whitespace()
        .last()
        .ok_or_else(|| "updated tree-ring binary did not print a version".to_string())?;
    Version::parse(version)
        .map_err(|error| format!("updated tree-ring printed an invalid version: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn parses_release_versions_with_or_without_v_prefix() {
        assert_eq!(
            parse_release_version("v0.15.0").unwrap(),
            Version::new(0, 15, 0)
        );
        assert_eq!(
            parse_release_version("0.15.1").unwrap(),
            Version::new(0, 15, 1)
        );
    }

    #[test]
    fn classifies_project_local_direct_and_homebrew_installs() {
        assert!(matches!(
            classify_install(Path::new("/work/demo/.tree-ring/bin/tree-ring")).unwrap(),
            InstallMethod::ProjectLocal { .. }
        ));
        assert!(matches!(
            classify_install(Path::new("/Users/example/.local/bin/tree-ring")).unwrap(),
            InstallMethod::Direct { .. }
        ));
        assert_eq!(
            classify_install(Path::new(
                "/opt/homebrew/Cellar/tree-ring/0.15.0/bin/tree-ring"
            ))
            .unwrap(),
            InstallMethod::Homebrew
        );
    }

    #[test]
    fn refuses_an_executable_without_a_bin_prefix() {
        let error = classify_install(Path::new("/work/demo/tree-ring")).unwrap_err();
        assert!(error.contains("outside a bin directory"));
    }

    #[test]
    fn selects_only_official_named_release_assets() {
        let release = GithubRelease {
            tag_name: "v0.15.0".to_string(),
            assets: vec![GithubAsset {
                name: "tree-ring-memory-0.15.0-darwin-arm64.tar.gz".to_string(),
                browser_download_url: format!(
                    "{RELEASE_DOWNLOAD_PREFIX}v0.15.0/tree-ring-memory-0.15.0-darwin-arm64.tar.gz"
                ),
            }],
        };
        assert!(asset_url(&release, "tree-ring-memory-0.15.0-darwin-arm64.tar.gz").is_ok());
        assert!(asset_url(&release, "missing.tar.gz").is_err());
    }

    #[test]
    fn rejects_release_assets_from_an_unexpected_host() {
        let release = GithubRelease {
            tag_name: "v0.15.0".to_string(),
            assets: vec![GithubAsset {
                name: "archive.tar.gz".to_string(),
                browser_download_url: "https://example.com/archive.tar.gz".to_string(),
            }],
        };
        assert!(asset_url(&release, "archive.tar.gz")
            .unwrap_err()
            .contains("unexpected download URL"));
    }

    #[test]
    fn parses_and_validates_sha256_files() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("archive.tar.gz");
        let mut file = File::create(&archive).unwrap();
        file.write_all(b"tree-ring").unwrap();
        let checksum = format!("{:x}", Sha256::digest(b"tree-ring"));

        assert_eq!(
            checksum_from_text(&format!("{checksum}  archive.tar.gz\n")).unwrap(),
            checksum
        );
        verify_checksum(&archive, &checksum).unwrap();
        assert!(checksum_from_text("not-a-checksum").is_err());
        assert!(verify_checksum(&archive, &"0".repeat(64)).is_err());
    }

    #[test]
    fn parses_tree_ring_version_output() {
        assert_eq!(
            parse_version_output(b"tree-ring 0.15.0\n", true).unwrap(),
            Version::new(0, 15, 0)
        );
        assert!(parse_version_output(b"tree-ring 0.15.0\n", false).is_err());
    }
}
