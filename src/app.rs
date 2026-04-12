use std::sync::Arc;

use crate::{
    config::AppConfig,
    errors::AppResult,
    mcp::{server::McpServer, tools::ToolRouter},
    providers::annas_archive::AnnasArchiveProvider,
    services::{
        download_service::DownloadService, open_service::OpenService, search_service::SearchService,
    },
};

pub struct App {
    server: McpServer,
}

impl App {
    pub async fn build(config: AppConfig) -> AppResult<Self> {
        config.validate()?;
        config.require_provider_credentials()?;

        let shared_config = Arc::new(config);

        let provider = Arc::new(AnnasArchiveProvider::new(shared_config.clone())?);
        let open_service = Arc::new(OpenService::new());

        let search_service = Arc::new(SearchService::new(provider.clone(), shared_config.clone()));

        let download_service = Arc::new(DownloadService::new(
            provider,
            open_service,
            shared_config.clone(),
        ));

        let tool_router = Arc::new(ToolRouter::new(
            search_service,
            download_service,
            shared_config.clone(),
        ));

        let server = McpServer::new(
            shared_config.server.name.clone(),
            shared_config.server.version.clone(),
            Some(build_instructions()),
            tool_router,
        );

        Ok(Self { server })
    }

    pub async fn run(self) -> AppResult<()> {
        self.server.run_stdio().await
    }
}

fn build_instructions() -> String {
    [
        "This MCP server searches for books and downloads selected results safely.",
        "Use `search_books` to find normalized results with stable `result_id` values.",
        "Use `download_book` to download a specific result by `result_id` or `provider_item_id`.",
        "Use `search_and_download_book` for a compatibility workflow that may auto-select only when confidence is high enough.",
        "Automatic file opening is opt-in.",
    ]
    .join(" ")
}
