use zed_extension_api as zed;
use zed::settings::LspSettings;

struct RustPhpExtension;

impl zed::Extension for RustPhpExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let settings = LspSettings::for_worktree("rust-php-lsp", worktree)?;
        let mut command = zed::Command::new(resolve_binary(&settings, worktree)?);
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
        Ok(LspSettings::for_worktree("rust-php-lsp", worktree)?.initialization_options)
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        Ok(LspSettings::for_worktree("rust-php-lsp", worktree)?.settings)
    }
}

fn resolve_binary(settings: &LspSettings, worktree: &zed::Worktree) -> zed::Result<String> {
    if let Some(path) = settings.binary.as_ref().and_then(|binary| binary.path.clone()) {
        return Ok(path);
    }

    if let Some(path) = worktree.which("rust-php") {
        return Ok(path);
    }

    Err(
        "could not find `rust-php` on PATH. Configure `lsp.rust-php-lsp.binary.path` in Zed or install the binary.".to_string(),
    )
}

zed::register_extension!(RustPhpExtension);
