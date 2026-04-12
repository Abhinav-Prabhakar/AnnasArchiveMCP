mod app;
mod config;
mod domain;
mod errors;
mod logging;
mod mcp;
mod providers;
mod services;
mod util;

use std::process::ExitCode;

use crate::app::App;
use crate::errors::AppError;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", render_error(&error));
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), AppError> {
    let _ = dotenvy::dotenv();

    let config = config::AppConfig::load()?;
    logging::init(&config.logging)?;

    tracing::info!(
        server = %config.server.name,
        version = %config.server.version,
        provider = %config.provider_name(),
        "starting MCP server"
    );

    let app = App::build(config).await?;
    app.run().await
}

fn render_error(error: &AppError) -> String {
    let mut message = error.user_message().to_string();

    if !message.ends_with('.') {
        message.push('.');
    }

    format!("annas-archive-mcp failed: {message}")
}
