// MCP server that runs inside the app, so machine learning (ML) clients can read and edit the node graph live
//
// it's served over streamable HTTP at `http://127.0.0.1:{MCP_PORT}/mcp`, loopback only
// connect any MCP client that speaks streamable HTTP to `http://127.0.0.1:6578/mcp` (server name: motionkeys)

mod tools;

use rmcp::transport::streamable_http_server::{session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService};

// TODO: make configurable from the settings window
pub const MCP_PORT: u16 = 6578;

/// runs the MCP server until the app exits, logs and returns if the port can't be bound
pub async fn start_mcp_server() {
    // the default config only accepts loopback Host headers, and enforcing origin validation with an
    // empty allow list also rejects any request coming from a web page
    let config = StreamableHttpServerConfig::default().enforce_origin_validation();
    // every MCP session gets its own tools instance, mounted at /mcp
    let service = StreamableHttpService::new(|| Ok(tools::MotionKeysMcp::new()), LocalSessionManager::default().into(), config);
    let router = axum::Router::new().nest_service("/mcp", service);

    // bind to loopback only, if the port is taken just log it and give up (the rest of the app keeps working)
    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", MCP_PORT)).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("MCP server could not bind 127.0.0.1:{}: {}", MCP_PORT, e);
            return;
        }
    };

    // serve until the app exits
    println!("MCP server listening on http://127.0.0.1:{}/mcp", MCP_PORT);
    if let Err(e) = axum::serve(listener, router).await {
        eprintln!("MCP server stopped: {}", e);
    }
}
