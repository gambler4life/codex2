use serde_json::json;
use std::fs;
use std::io;
use std::path::Path;
use toml::Value;
use toml::map::Map;

const PROFILE_SUFFIX: &str = ".config.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProviderSetupCommand {
    List,
    Add {
        id: String,
        base_url: String,
        model: String,
        wire_api: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderProfileSummary {
    pub(crate) id: String,
    pub(crate) model: Option<String>,
    pub(crate) provider_name: Option<String>,
    pub(crate) wire_api: Option<String>,
    pub(crate) has_inline_key: bool,
}

pub(crate) fn parse_provider_setup_args(args: &str) -> Result<ProviderSetupCommand, String> {
    let parts = args.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        return Ok(ProviderSetupCommand::List);
    }
    match parts.as_slice() {
        ["add", id, base_url, model] => Ok(ProviderSetupCommand::Add {
            id: validate_profile_id(id)?,
            base_url: validate_non_empty("base_url", base_url)?,
            model: validate_non_empty("model", model)?,
            wire_api: "chat".to_string(),
        }),
        ["add", id, base_url, model, wire_api] => {
            let wire_api = validate_wire_api(wire_api)?;
            Ok(ProviderSetupCommand::Add {
                id: validate_profile_id(id)?,
                base_url: validate_non_empty("base_url", base_url)?,
                model: validate_non_empty("model", model)?,
                wire_api,
            })
        }
        _ => Err("Usage: /providers [add <id> <base_url> <model> [chat|responses]]".to_string()),
    }
}

pub(crate) fn parse_api_key_args(args: &str) -> Result<(String, String), String> {
    let mut parts = args.split_whitespace();
    let Some(id) = parts.next() else {
        return Err("Usage: /api-key <provider> <api_key>".to_string());
    };
    let Some(api_key) = parts.next() else {
        return Err("Usage: /api-key <provider> <api_key>".to_string());
    };
    if parts.next().is_some() {
        return Err("Usage: /api-key <provider> <api_key>".to_string());
    }
    Ok((validate_profile_id(id)?, api_key.to_string()))
}

pub(crate) fn list_provider_profiles(codex_home: &Path) -> io::Result<Vec<ProviderProfileSummary>> {
    let mut profiles = Vec::new();
    let Ok(entries) = fs::read_dir(codex_home) else {
        return Ok(profiles);
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(id) = file_name.strip_suffix(PROFILE_SUFFIX) else {
            continue;
        };
        if id == "config" {
            continue;
        }
        let contents = fs::read_to_string(&path).unwrap_or_default();
        let value = toml::from_str::<Value>(&contents).ok();
        profiles.push(ProviderProfileSummary {
            id: id.to_string(),
            model: value
                .as_ref()
                .and_then(|value| value.get("model"))
                .and_then(Value::as_str)
                .map(str::to_string),
            provider_name: provider_table(value.as_ref(), id)
                .and_then(|provider| provider.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string),
            wire_api: provider_table(value.as_ref(), id)
                .and_then(|provider| provider.get("wire_api"))
                .and_then(Value::as_str)
                .map(str::to_string),
            has_inline_key: provider_table(value.as_ref(), id)
                .and_then(|provider| provider.get("experimental_bearer_token"))
                .and_then(Value::as_str)
                .is_some_and(|key| !key.is_empty()),
        });
    }
    profiles.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(profiles)
}

pub(crate) fn add_provider_profile(
    codex_home: &Path,
    id: &str,
    base_url: &str,
    model: &str,
    wire_api: &str,
) -> io::Result<()> {
    let id = validate_profile_id(id).map_err(invalid_input)?;
    let base_url = validate_non_empty("base_url", base_url).map_err(invalid_input)?;
    let model = validate_non_empty("model", model).map_err(invalid_input)?;
    let wire_api = validate_wire_api(wire_api).map_err(invalid_input)?;
    fs::create_dir_all(codex_home)?;
    let models_dir = codex_home.join("models");
    fs::create_dir_all(&models_dir)?;
    let catalog_path = models_dir.join(format!("{id}.models.json"));
    fs::write(
        &catalog_path,
        serde_json::to_string_pretty(&single_model_catalog(&model)).map_err(io::Error::other)?,
    )?;
    let env_key = format!("{}_API_KEY", id.replace('-', "_").to_ascii_uppercase());
    let mut provider = Map::new();
    provider.insert("name".to_string(), Value::String(id.clone()));
    provider.insert("base_url".to_string(), Value::String(base_url));
    provider.insert("env_key".to_string(), Value::String(env_key.clone()));
    provider.insert(
        "env_key_instructions".to_string(),
        Value::String(format!(
            "Run /api-key {id} <key>, or set {env_key} before launching codex2."
        )),
    );
    provider.insert("wire_api".to_string(), Value::String(wire_api));
    provider.insert("requires_openai_auth".to_string(), Value::Boolean(false));
    provider.insert("supports_websockets".to_string(), Value::Boolean(false));

    let mut providers = Map::new();
    providers.insert(id.clone(), Value::Table(provider));

    let mut root = Map::new();
    root.insert("model_provider".to_string(), Value::String(id.clone()));
    root.insert("model".to_string(), Value::String(model));
    root.insert(
        "model_catalog_json".to_string(),
        Value::String(format!("models/{id}.models.json")),
    );
    root.insert(
        "check_for_update_on_startup".to_string(),
        Value::Boolean(false),
    );
    root.insert("model_providers".to_string(), Value::Table(providers));

    fs::write(
        codex_home.join(format!("{id}{PROFILE_SUFFIX}")),
        toml::to_string_pretty(&Value::Table(root)).map_err(io::Error::other)?,
    )
}

pub(crate) fn store_provider_api_key(codex_home: &Path, id: &str, api_key: &str) -> io::Result<()> {
    let id = validate_profile_id(id).map_err(invalid_input)?;
    let path = codex_home.join(format!("{id}{PROFILE_SUFFIX}"));
    let contents = fs::read_to_string(&path)?;
    let mut value =
        toml::from_str::<Value>(&contents).map_err(|err| invalid_input(err.to_string()))?;
    let Some(provider) = value
        .get_mut("model_providers")
        .and_then(Value::as_table_mut)
        .and_then(|providers| providers.get_mut(&id))
        .and_then(Value::as_table_mut)
    else {
        return Err(invalid_input(format!(
            "Profile '{id}' does not define [model_providers.{id}]"
        )));
    };
    provider.insert(
        "experimental_bearer_token".to_string(),
        Value::String(api_key.to_string()),
    );
    fs::write(
        &path,
        toml::to_string_pretty(&value).map_err(io::Error::other)?,
    )
}

fn provider_table<'a>(
    value: Option<&'a Value>,
    id: &str,
) -> Option<&'a toml::map::Map<String, Value>> {
    value?
        .get("model_providers")?
        .as_table()?
        .get(id)?
        .as_table()
}

fn validate_profile_id(id: &str) -> Result<String, String> {
    let valid = !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if valid {
        Ok(id.to_ascii_lowercase())
    } else {
        Err("Provider id must use only letters, numbers, '-' or '_'.".to_string())
    }
}

fn validate_wire_api(wire_api: &str) -> Result<String, String> {
    match wire_api {
        "chat" | "responses" => Ok(wire_api.to_string()),
        _ => Err("wire API must be either 'chat' or 'responses'.".to_string()),
    }
}

fn validate_non_empty(name: &str, value: &str) -> Result<String, String> {
    if value.is_empty() {
        Err(format!("{name} must not be empty."))
    } else {
        Ok(value.to_string())
    }
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn single_model_catalog(model: &str) -> serde_json::Value {
    json!({
        "models": [{
            "slug": model,
            "display_name": model,
            "description": "Custom model added from /providers.",
            "supported_reasoning_levels": [],
            "shell_type": "shell_command",
            "visibility": "list",
            "supported_in_api": true,
            "priority": 0,
            "additional_speed_tiers": [],
            "service_tiers": [],
            "availability_nux": null,
            "upgrade": null,
            "base_instructions": "You are Codex, a pragmatic coding agent. Work carefully, use tools when useful, and provide concise engineering updates.",
            "supports_reasoning_summaries": false,
            "default_reasoning_summary": "none",
            "support_verbosity": false,
            "default_verbosity": null,
            "apply_patch_tool_type": null,
            "web_search_tool_type": "text",
            "truncation_policy": {"mode": "tokens", "limit": 10000},
            "supports_parallel_tool_calls": false,
            "supports_image_detail_original": false,
            "context_window": 128000,
            "max_context_window": 128000,
            "effective_context_window_percent": 90,
            "experimental_supported_tools": [],
            "input_modalities": ["text"],
            "supports_search_tool": false
        }]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parses_add_provider_with_default_chat_wire_api() {
        assert_eq!(
            parse_provider_setup_args("add acme https://api.example.com/v1 acme-model").unwrap(),
            ProviderSetupCommand::Add {
                id: "acme".to_string(),
                base_url: "https://api.example.com/v1".to_string(),
                model: "acme-model".to_string(),
                wire_api: "chat".to_string(),
            }
        );
    }

    #[test]
    fn rejects_path_like_provider_id() {
        assert!(parse_provider_setup_args("add ../bad https://api.example.com/v1 m").is_err());
    }

    #[test]
    fn writes_profile_catalog_and_inline_key() {
        let temp_dir = TempDir::new().unwrap();
        add_provider_profile(
            temp_dir.path(),
            "acme",
            "https://api.example.com/v1",
            "acme-model",
            "chat",
        )
        .unwrap();
        store_provider_api_key(temp_dir.path(), "acme", "sk-test").unwrap();

        let profile = fs::read_to_string(temp_dir.path().join("acme.config.toml")).unwrap();
        assert!(profile.contains("model_provider = \"acme\""));
        assert!(profile.contains("experimental_bearer_token = \"sk-test\""));
        let catalog = fs::read_to_string(temp_dir.path().join("models/acme.models.json")).unwrap();
        assert!(catalog.contains("acme-model"));
        let profiles = list_provider_profiles(temp_dir.path()).unwrap();
        assert_eq!(profiles.len(), 1);
        assert!(profiles[0].has_inline_key);
    }
}
