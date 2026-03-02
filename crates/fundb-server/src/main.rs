//! FunDB server entry point — PostgreSQL wire-protocol server with real
//! query execution (parse → bind → execute → storage), plus HTTP REST API.
//!
//! Listens on port 5433 (PG wire) and 8080 (HTTP REST API).

use std::sync::Arc;

use fundb_executor::Executor;
use fundb_protocol::PgConnection;
use fundb_sql::binder::Catalog;
use fundb_storage::LsmTree;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

mod handler;
mod http;
mod stub_handler;
use handler::FunDBHandler;
use http::HttpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Open storage
    let data_dir = std::path::PathBuf::from("./fundb_data");
    std::fs::create_dir_all(&data_dir)?;
    let lsm = LsmTree::open(&data_dir, 64 * 1024 * 1024)?; // 64MB memtable
    let lsm = Arc::new(lsm);
    tracing::info!("Storage opened at {}", data_dir.display());

    // Catalog with default collections
    let catalog = Arc::new(RwLock::new(Catalog::open()));

    // Executor + handler
    let executor = Executor::new(Arc::clone(&lsm));
    let handler = FunDBHandler::new(executor, catalog);
    let handler: Arc<dyn fundb_protocol::QueryHandler> = Arc::new(handler);

    // Start HTTP REST API server on port 8080
    let http_server = HttpServer::new("0.0.0.0:8080");
    tokio::spawn(async move {
        if let Err(e) = http_server.run().await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    // Start PG wire-protocol server on port 5433
    let addr = "0.0.0.0:5433";
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("FunDB listening on {}", addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        tracing::info!("New connection from {}", peer_addr);
        let handler = handler.clone();
        tokio::spawn(async move {
            let conn = PgConnection::new(stream);
            if let Err(e) = conn.run(handler).await {
                tracing::error!("Connection error from {}: {}", peer_addr, e);
            }
        });
    }
}
