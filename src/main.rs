use std::{fs::File, io::BufReader, net::SocketAddr, path::Path};

use color_eyre::eyre::{self, WrapErr};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::rustls;
use tracing::instrument;
use tracing_subscriber::{fmt, prelude::*, Registry};

fn load_certs<P: AsRef<Path>>(path: P) -> eyre::Result<Vec<rustls::Certificate>> {
    let path = path.as_ref();
    let f = File::open(path).wrap_err_with(|| format!("path {}", path.display()))?;
    let mut f = BufReader::new(f);
    let certs = rustls_pemfile::certs(&mut f)?;
    Ok(certs.into_iter().map(rustls::Certificate).collect())
}

#[instrument(skip_all, fields(%addr))]
async fn handle_connection(tcp_stream: TcpStream, addr: SocketAddr) {
    tracing::info!("Incoming TCP connection");
    let _ws_stream = tokio_tungstenite::accept_async(tcp_stream).await.unwrap();
    tracing::debug!("WebSocket connection established");
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_init();

    let certs = load_certs(concat!(env!("CARGO_MANIFEST_DIR"), "/test.cer"))?;
    assert_eq!(certs.len(), 1);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;

    let _server = tokio::spawn(async move {
        while let Ok((stream, addr)) = listener.accept().await {
            tokio::spawn(handle_connection(stream, addr));
        }
    });

    let url = format!("ws://{local_addr}");
    let client = tokio::spawn(async move {
        let (_ws_stream, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        tracing::info!("✅ WebSocket connection established");
    });

    client.await?;
    Ok(())
}

fn tracing_init() {
    let fmt_layer = fmt::layer().with_line_number(true);
    let subscriber = Registry::default().with(fmt_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
}
