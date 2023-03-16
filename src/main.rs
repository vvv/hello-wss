use std::{fs::File, io::BufReader, net::SocketAddr, path::Path, sync::Arc};

use color_eyre::eyre::{self, WrapErr};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
};
use tokio_rustls::{rustls, TlsAcceptor};
use tracing::instrument;

fn open_file<P: AsRef<Path>>(path: P) -> eyre::Result<BufReader<File>> {
    let path = path.as_ref();
    let f = File::open(path).wrap_err_with(|| format!("path {}", path.display()))?;
    Ok(BufReader::new(f))
}

fn load_certs<P: AsRef<Path>>(path: P) -> eyre::Result<Vec<rustls::Certificate>> {
    let bufs = rustls_pemfile::certs(&mut open_file(path)?)?;
    Ok(bufs.into_iter().map(rustls::Certificate).collect())
}

fn load_keys<P: AsRef<Path>>(path: P) -> eyre::Result<Vec<rustls::PrivateKey>> {
    let bufs = rustls_pemfile::pkcs8_private_keys(&mut open_file(path)?)?;
    Ok(bufs.into_iter().map(rustls::PrivateKey).collect())
}

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

    let certs = load_certs(concat!(env!("CARGO_MANIFEST_DIR"), "/test.cer"))?;
    assert_eq!(certs.len(), 1);

    let mut keys = load_keys(concat!(env!("CARGO_MANIFEST_DIR"), "/test.key"))?;
    assert_eq!(keys.len(), 1);

    let config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        // XXX-REVIEW: Note that the end-entity certificate must have the
        // Subject Alternative Name extension to describe, e.g., the valid DNS
        // name.
        //
        // See https://docs.rs/rustls/0.20.8/rustls/struct.ConfigBuilder.html#method.with_single_cert-2
        .with_single_cert(certs, keys.remove(0))?;
    let acceptor = TlsAcceptor::from(Arc::new(config));

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
