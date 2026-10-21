use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use tokio::{net::TcpListener, sync::Semaphore, task::JoinSet, time::timeout};

const MAX_CONNECTIONS: usize = 64;
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, Parser)]
#[command(version, about = "Aegis v0.1 mutually authenticated TLS echo server")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8443")]
    listen: SocketAddr,
    /// Dedicated device certificate authority PEM.
    #[arg(long)]
    ca: PathBuf,
    /// Server certificate chain PEM.
    #[arg(long)]
    cert: PathBuf,
    /// Server private key PEM; never commit this file.
    #[arg(long)]
    key: PathBuf,
}

async fn shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            received = terminate.recv() => {
                received.ok_or_else(|| std::io::Error::other("signal handler closed"))
            }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let tls = aegis_transports::server_config(&args.ca, &args.cert, &args.key)?;
    let listener = TcpListener::bind(args.listen).await?;
    let permits = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let mut sessions = JoinSet::new();
    // The configured local listening endpoint is safe to show. Peer identifiers,
    // certificate identities, credentials and user traffic are never logged.
    println!(
        "Aegis v0.1 listening on {} (mutual TLS)",
        listener.local_addr()?
    );

    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            result = &mut shutdown => {
                result?;
                break;
            }
            result = listener.accept() => {
                let (tcp, _) = result?;
                // Include finished-but-not-yet-reaped tasks in the bound too.
                if sessions.len() >= MAX_CONNECTIONS {
                    continue;
                }
                let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
                    // Drop overload connections immediately; never spawn an
                    // unbounded queue waiting for admission.
                    continue;
                };
                let tls = Arc::clone(&tls);
                sessions.spawn(async move {
                    let _permit = permit;
                    let Ok(stream) = aegis_transports::accept(tcp, tls).await else {
                        return;
                    };
                    let _ = aegis_core::serve_echo(stream).await;
                });
            }
            Some(result) = sessions.join_next(), if !sessions.is_empty() => {
                if result.is_err() {
                    eprintln!("Aegis session task terminated unexpectedly");
                }
            }
        }
    }
    drop(listener);
    // Give established sessions a small, bounded opportunity to finish. Abort
    // and reap every remaining task when the grace period expires.
    if timeout(SHUTDOWN_GRACE, async {
        while sessions.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        sessions.abort_all();
        while sessions.join_next().await.is_some() {}
    }
    println!("Aegis server stopped");
    Ok(())
}
