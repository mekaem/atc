use miette::Diagnostic;
use owo_colors::OwoColorize;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    #[error("{}", format_error("Configuration error", .0))]
    Config(String),

    #[error("{}", format_error("I/O error", .0.to_string()))]
    Io(#[from] std::io::Error),

    #[error("{}", format_error("TOML error", .0.to_string()))]
    Toml(#[from] toml::de::Error),

    #[error("{}", format_error("Network error", .0))]
    Network(String),

    #[error("{}", format_error("Certificate error", .0))]
    Cert(String),

    #[error("{}", format_error("API error", .0))]
    Api(String),

    #[error("{}", format_error("JSON error", .0.to_string()))]
    Json(#[from] serde_json::Error),
}

fn format_error(error_type: &str, message: impl AsRef<str>) -> String {
    format!(
        "{} {} {}",
        "→".red().bold(),
        error_type.bright_red().bold(),
        message.as_ref().yellow()
    )
}

pub type Result<T> = std::result::Result<T, Error>;
