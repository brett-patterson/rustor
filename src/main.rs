mod client;
mod torrent;
mod torrent_file;
mod tracker;
mod types;
mod worker;
mod writer;

use std::path::Path;

use clap::{command, Parser};
use client::TorrentClient;
use torrent_file::TorrentMetaInfo;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    filename: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let client = TorrentClient::new(6881);
    let torrent_file = TorrentMetaInfo::from_file(Path::new(&args.filename))?;
    client.download_file(torrent_file).await
}
