use aegis_client::{MAX_INPUT_BYTES, roundtrip};
use clap::Parser;
use std::{error::Error, io::Read, path::PathBuf};

#[derive(Parser)]
#[command(version, about = "Aegis v0.1 authenticated TLS echo client")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8443")]
    server: String,
    /// DNS name checked against the server certificate.
    #[arg(long, default_value = "localhost")]
    server_name: String,
    /// Dedicated certificate authority PEM used to authenticate the server.
    #[arg(long)]
    ca: PathBuf,
    /// Client certificate chain PEM issued by the dedicated device authority.
    #[arg(long)]
    cert: PathBuf,
    /// Client private key PEM. Never include this file in a repository.
    #[arg(long)]
    key: PathBuf,
    #[arg(long, default_value = "Hello, Aegis!", conflicts_with = "input")]
    message: String,
    /// Binary input; bounded to 1 MiB before allocating the verification buffer.
    #[arg(long)]
    input: Option<PathBuf>,
    /// Optional output file; written only after the complete exchange succeeds.
    #[arg(long)]
    output: Option<PathBuf>,
}

fn read_input(args: &Args) -> Result<Vec<u8>, Box<dyn Error>> {
    let bytes = if let Some(path) = &args.input {
        let file = std::fs::File::open(path)?;
        let mut bytes = Vec::new();
        // Reading a cap+1 sentinel also bounds non-regular files and avoids
        // trusting a metadata size that may change between stat and read.
        file.take(MAX_INPUT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        bytes
    } else {
        args.message.as_bytes().to_vec()
    };
    if bytes.len() > MAX_INPUT_BYTES {
        return Err("input exceeds the 1 MiB prototype limit".into());
    }
    Ok(bytes)
}

async fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let input = read_input(&args)?;
    let config = aegis_transports::client_config(&args.ca, &args.cert, &args.key)?;
    let io = aegis_transports::connect(&args.server, &args.server_name, config).await?;
    let verified = roundtrip(io, &input).await?;
    if let Some(path) = args.output {
        std::fs::write(path, &verified)?;
    }
    println!(
        "Verified {} bytes over Aegis v0.1; session closed cleanly.",
        verified.len()
    );
    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(error) = run(args).await {
        eprintln!("Aegis client failed: {error}");
        std::process::exit(1);
    }
}
