//! FunDB server entry point — STORY-1-4 PostgreSQL wire-protocol skeleton.
//!
//! Listens on port 5433 and accepts connections from any PostgreSQL-compatible
//! client (e.g. `psql -h 127.0.0.1 -p 5433 -U fun fundb`).

use std::sync::Arc;

use fundb_protocol::PgConnection;
use tokio::net::TcpListener;

mod stub_handler;
use stub_handler::StubHandler;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let addr = "0.0.0.0:5433";
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("FunDB listening on {}", addr);

    let handler: Arc<dyn fundb_protocol::QueryHandler> = Arc::new(StubHandler);

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
