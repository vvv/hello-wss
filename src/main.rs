use std::net::SocketAddr;

use color_eyre::eyre;
use obfstr::obfstr;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
};
use tracing::instrument;

#[instrument(skip_all, fields(%addr))]
async fn handle_connection<S>(stream: S, addr: SocketAddr) -> eyre::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    tracing::info!("Incoming TCP connection");
    let _ws_stream = tokio_tungstenite::accept_async(stream).await?;
    tracing::debug!("WebSocket connection established");
    Ok(())
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    tracing_init();

    let acceptor = {
        let p12 = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/identity.p12"));
        let identity = native_tls::Identity::from_pkcs12(p12, obfstr!("mypass"))?;
        tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::new(identity)?)
    };
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;

    let server = tokio::spawn(async move {
        while let Ok((tcp_stream, addr)) = listener.accept().await {
            let tls_stream = acceptor.accept(tcp_stream).await?;
            tokio::spawn(handle_connection(tls_stream, addr));
        }
        Ok::<_, eyre::Report>(())
    });

    let url = format!("wss://{local_addr}");
    let client = tokio::spawn(async move {
        let (_ws_stream, _) = tokio_tungstenite::connect_async(url).await?;
        tracing::info!("✅ WebSocket connection established");
        Ok::<_, eyre::Report>(())
    });

    client.await.unwrap()?;
    server.await.unwrap()?;
    Ok(())
}

fn tracing_init() {
    use tracing_subscriber::{fmt, prelude::*, Registry};

    let fmt_layer = fmt::layer().with_line_number(true);
    let subscriber = Registry::default().with(fmt_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
}
