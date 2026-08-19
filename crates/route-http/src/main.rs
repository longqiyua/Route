//! Route HTTP REST API server — entry point.
//!
//! # Usage
//!
//! ```bash
//! # Start on default port 8080
//! route-http
//!
//! # Start on a custom port
//! route-http --port 3000
//!
//! # Start in a specific project directory
//! route-http --path /path/to/project
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use route_http::{build_router, AppState};

#[derive(Parser)]
#[command(name = "route-http", version, about = "Route HTTP REST API server")]
struct Cli {
    /// Port to listen on (default: 8080)
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Host address to bind to (default: 0.0.0.0)
    #[arg(short, long, default_value = "0.0.0.0")]
    host: String,

    /// Project path (default: current working directory)
    #[arg(short, long)]
    path: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    // Initialize tracing (logging)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let project_path = cli
        .path
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    tracing::info!(
        "Starting Route HTTP API server on {}:{} (project: {})",
        cli.host,
        cli.port,
        project_path.display(),
    );

    // Build shared state and router
    let state = Arc::new(AppState::new(project_path));
    let app = build_router(state);

    let addr = format!("{}:{}", cli.host, cli.port);
    let listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!("Listening on http://{}", addr);

    // Start the server
    axum::serve(listener, app).await.expect("Server failed");
}
