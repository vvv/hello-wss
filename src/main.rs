use std::{fs, net::SocketAddr, path::Path};

use color_eyre::{
    eyre::{self, WrapErr as _},
    Section as _,
};
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
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("identity.p12");
        let p12 = fs::read(&path)
            .wrap_err_with(|| format!("Cannot read {}", path.display()))
            .suggestion("run 'scripts/generate-certificate' to create 'identity.p12'")?;
        let identity = native_tls::Identity::from_pkcs12(&p12, obfstr!("mypass"))?;
        tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::new(identity)?)
    };
    #[cfg(feature = "connect")]
    let port = 0;
    #[cfg(not(feature = "connect"))]
    let port = 8443;
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    #[cfg(feature = "connect")]
    let url = format!("wss://{}", listener.local_addr()?);

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

    #[cfg(feature = "connect")]
    let client = tokio::spawn(async move {
        let (_ws_stream, _) = tokio_tungstenite::connect_async(url)
            .await
            .wrap_err("connect_async failed")?;
        tracing::info!("✅ WebSocket connection established");
        Ok::<_, eyre::Report>(())
    });

    #[cfg(feature = "connect")]
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
