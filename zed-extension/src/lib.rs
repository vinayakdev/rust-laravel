use zed::settings::LspSettings;
use zed::{
    current_platform, Architecture, DownloadedFileType, GithubRelease, GithubReleaseAsset,
    GithubReleaseOptions, LanguageServerInstallationStatus, Os,
};
use zed_extension_api as zed;

const LANGUAGE_SERVER_ID: &str = "rust-php-lsp";
const BINARY_NAME: &str = "rust-php";
const REPOSITORY: &str = "vinayakdev/rust-laravel";

struct RustPhpExtension;

impl zed::Extension for RustPhpExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let settings = LspSettings::for_worktree(LANGUAGE_SERVER_ID, worktree)?;
        let mut command =
            zed::Command::new(resolve_binary(language_server_id, &settings, worktree)?);
        command = command.arg("lsp");

        if let Some(binary) = settings.binary {
            if let Some(arguments) = binary.arguments {
                command = command.args(arguments);
            }
            if let Some(env) = binary.env {
                command = command.envs(env);
            }
        }

        Ok(command)
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        Ok(LspSettings::for_worktree(LANGUAGE_SERVER_ID, worktree)?.initialization_options)
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        Ok(LspSettings::for_worktree(LANGUAGE_SERVER_ID, worktree)?.settings)
    }
}

fn resolve_binary(
    language_server_id: &zed::LanguageServerId,
    settings: &LspSettings,
    worktree: &zed::Worktree,
) -> zed::Result<String> {
    if let Some(path) = settings
        .binary
        .as_ref()
        .and_then(|binary| binary.path.clone())
    {
        return Ok(path);
    }

    if let Some(bundled) = bundled_binary_path() {
        if binary_is_available(&bundled) {
            return Ok(bundled);
        }
    }

    if let Some(path) = worktree.which(BINARY_NAME) {
        return Ok(path);
    }

    install_release_binary(language_server_id)
}

fn bundled_binary_path() -> Option<String> {
    let (os, arch) = current_platform();
    let name = match (os, arch) {
        (Os::Mac, Architecture::Aarch64) => "rust-php-macos-aarch64",
        (Os::Mac, Architecture::X8664) => "rust-php-macos-x86_64",
        (Os::Linux, Architecture::X8664) => "rust-php-linux-x86_64",
        (Os::Windows, Architecture::X8664) => "rust-php-windows-x86_64.exe",
        _ => return None,
    };
    Some(format!("bin/{name}"))
}

fn install_release_binary(language_server_id: &zed::LanguageServerId) -> zed::Result<String> {
    let asset = release_asset_for_platform()?;
    let relative_path = downloaded_binary_path(&asset);

    if binary_is_available(&relative_path) {
        return Ok(relative_path);
    }

    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::CheckingForUpdate,
    );

    let release = release_for_current_version().map_err(|error| {
        set_install_failed(language_server_id, &error);
        error
    })?;
    let release_asset = find_release_asset(&release, &asset.name).map_err(|error| {
        set_install_failed(language_server_id, &error);
        error
    })?;

    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::Downloading,
    );

    let result = zed::download_file(
        &release_asset.download_url,
        &relative_path,
        DownloadedFileType::Uncompressed,
    )
    .and_then(|()| {
        if asset.make_executable {
            zed::make_file_executable(&relative_path)?;
        }
        Ok(relative_path)
    });

    match result {
        Ok(path) => {
            zed::set_language_server_installation_status(
                language_server_id,
                &LanguageServerInstallationStatus::None,
            );
            Ok(path)
        }
        Err(error) => {
            set_install_failed(language_server_id, &error);
            Err(error)
        }
    }
}

fn release_for_current_version() -> zed::Result<GithubRelease> {
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    zed::github_release_by_tag_name(REPOSITORY, &tag).or_else(|_| {
        zed::latest_github_release(
            REPOSITORY,
            GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )
    })
}

fn find_release_asset<'a>(
    release: &'a GithubRelease,
    asset_name: &str,
) -> zed::Result<&'a GithubReleaseAsset> {
    release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| {
            format!(
                "release {} in {} does not contain the asset `{asset_name}`",
                release.version, REPOSITORY
            )
        })
}

fn binary_is_available(path: &str) -> bool {
    let mut command = zed::Command::new(path);
    command = command.arg("--version");
    command.output().is_ok()
}

fn set_install_failed(language_server_id: &zed::LanguageServerId, error: &str) {
    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::Failed(error.to_string()),
    );
}

struct ReleaseAssetSpec {
    name: &'static str,
    make_executable: bool,
}

fn release_asset_for_platform() -> zed::Result<ReleaseAssetSpec> {
    let (os, arch) = current_platform();
    match (os, arch) {
        (Os::Mac, Architecture::Aarch64) => Ok(ReleaseAssetSpec {
            name: "rust-php-macos-aarch64",
            make_executable: true,
        }),
        (Os::Mac, Architecture::X8664) => Ok(ReleaseAssetSpec {
            name: "rust-php-macos-x86_64",
            make_executable: true,
        }),
        (Os::Linux, Architecture::X8664) => Ok(ReleaseAssetSpec {
            name: "rust-php-linux-x86_64",
            make_executable: true,
        }),
        (Os::Windows, Architecture::X8664) => Ok(ReleaseAssetSpec {
            name: "rust-php-windows-x86_64.exe",
            make_executable: false,
        }),
        _ => Err(format!(
            "rust-laravel does not publish a prebuilt `{os:?}` `{arch:?}` binary yet. Install `rust-php` manually and configure `lsp.{LANGUAGE_SERVER_ID}.binary.path`."
        )),
    }
}

fn downloaded_binary_path(asset: &ReleaseAssetSpec) -> String {
    format!("rust-php/v{}/{}", env!("CARGO_PKG_VERSION"), asset.name)
}

zed::register_extension!(RustPhpExtension);

#[cfg(test)]
mod tests {
    use super::{downloaded_binary_path, ReleaseAssetSpec};

    #[test]
    fn downloads_are_version_scoped() {
        let asset = ReleaseAssetSpec {
            name: "rust-php-linux-x86_64",
            make_executable: true,
        };

        assert_eq!(
            downloaded_binary_path(&asset),
            format!(
                "rust-php/v{}/rust-php-linux-x86_64",
                env!("CARGO_PKG_VERSION")
            )
        );
    }
}
