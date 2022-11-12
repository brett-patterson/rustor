mod client;
mod torrent;
mod torrent_file;
mod tracker;
mod types;
mod worker;

use std::path::Path;

use client::TorrentClient;
use torrent_file::TorrentFile;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let client = TorrentClient::new(6881);
    // TODO: better arg handling
    let filename = std::env::args()
        .skip(1)
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invalid args"))?;

    let torrent_file = TorrentFile::from_file(Path::new(&filename))?;
    client.download_file(torrent_file).await
}
