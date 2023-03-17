use std::net::SocketAddr;

use color_eyre::eyre::{self, WrapErr};
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
    let _ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .wrap_err("accept_async failed")?;
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
            let tls_stream = acceptor
                .accept(tcp_stream)
                .await
                .wrap_err("accept failed")?;
            tokio::spawn(handle_connection(tls_stream, addr));
        }
        Ok::<_, eyre::Report>(())
    });

    let connector = {
        let cert = native_tls::Certificate::from_pem(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/_cert.pem"
        )))?;
        let connector = native_tls::TlsConnector::builder()
            .add_root_certificate(cert)
            // .use_sni(false)
            .build()?;
        tokio_tungstenite::Connector::NativeTls(connector)
    };

    let url = format!("wss://{local_addr}");
    let client = tokio::spawn(async move {
        // let (_ws_stream, _) = tokio_tungstenite::connect_async(url)
        //     .await
        //     .wrap_err("connect_async failed")?;
        let (_ws_stream, _) =
            tokio_tungstenite::connect_async_tls_with_config(url, None, Some(connector))
                .await
                .wrap_err("connect_async_tls_with_config failed")?;
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
