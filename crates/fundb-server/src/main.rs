//! FunDB server entry point — PostgreSQL wire-protocol server with real
//! query execution (parse → bind → execute → storage).
//!
//! Listens on port 5433 and accepts connections from any PostgreSQL-compatible
//! client (e.g. `psql -h 127.0.0.1 -p 5433 -U fun fundb`).

use std::sync::Arc;

use fundb_executor::Executor;
use fundb_protocol::PgConnection;
use fundb_sql::binder::Catalog;
use fundb_storage::LsmTree;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

mod handler;
mod stub_handler;
use handler::FunDBHandler;

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
