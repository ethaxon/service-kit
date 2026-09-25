use figment::{
    Figment,
    providers::{Format, Json, Toml, Yaml},
};
use service_kit_core::{Error, ErrorCode, Result, ServiceSpec};
use std::path::Path;

pub fn load_service_spec(path: &Path) -> Result<ServiceSpec> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "toml" | "json" | "yaml" | "yml") {
        return Err(Error::new(
            ErrorCode::InvalidSpec,
            "Unsupported specification extension; use .toml, .json, .yaml or .yml",
        ));
    }
    // Read explicitly: missing files must fail, never act as optional providers.
    let source = std::fs::read_to_string(path)?;
    let config = match extension.as_str() {
        "toml" => Figment::from(Toml::string(&source)),
        "json" => Figment::from(Json::string(&source)),
        _ => Figment::from(Yaml::string(&source)),
    };
    let spec: ServiceSpec = config.extract().map_err(|error| {
        Error::new(
            ErrorCode::InvalidSpec,
            format!("{}: {error}", path.display()),
        )
    })?;
    Ok(spec)
}
