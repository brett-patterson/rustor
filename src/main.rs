use std::path::Path;

use client::TorrentClient;
use torrent_file::TorrentFile;

mod client;
mod message;
mod torrent;
mod torrent_file;
mod tracker;
mod types;
mod worker;

#[tokio::main]
async fn main() {
    let client = TorrentClient::new(6881);
    if let Some(filename) = std::env::args().skip(1).next() {
        if let Result::Ok(torrent_file) = TorrentFile::from_file(Path::new(&filename)) {
            match client.download_file(torrent_file).await {
                Ok(_) => {}
                Err(error) => println!("{:?}", error),
            }
        }
    }
}
