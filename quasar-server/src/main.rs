use crate::server::QuasarServer;
use clap::Parser;
use std::net::SocketAddr;

mod channel;
mod code_generator;
mod error;
mod protocol;
mod server;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    address: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let server = QuasarServer::new(args.address);
    server.run().await?;

    Ok(())
}
