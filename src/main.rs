use std::fmt::Display;
use std::net::{Ipv4Addr, Shutdown, TcpStream};
use std::time::Duration;
use std::{error::Error, path::Path};

use rand::Rng;
use serde_bytes::ByteBuf;
use serde_derive::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

#[derive(Debug, Deserialize)]
struct TorrentFile {
    announce: String,
    info: TorrentFileInfo,
}

#[derive(Debug, Deserialize, Serialize)]
struct TorrentFileInfo {
    name: String,
    length: i64,
    pieces: ByteBuf,
    #[serde(rename = "piece length")]
    piece_length: i64,
}

#[derive(Debug, Deserialize)]
struct TrackerResponse {
    peers: ByteBuf,
}

#[derive(Debug)]
struct Peer {
    ip: Ipv4Addr,
    port: u16,
}

impl Display for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}:{}", self.ip, self.port))?;
        Result::Ok(())
    }
}

impl Peer {
    fn new(ip: Ipv4Addr, port: u16) -> Self {
        Peer { ip, port }
    }
}

struct TorrentClient {
    peer_id: Vec<u8>,
    port: u16,
}

impl TorrentClient {
    fn new(port: u16) -> Self {
        let peer_id: Vec<u8> = rand::thread_rng()
            .sample_iter(rand::distributions::Standard)
            .take(20)
            .collect();
        Self { peer_id, port }
    }

    fn download_file(&self, torrent_file: &TorrentFile) -> Result<(), Box<dyn Error>> {
        let peers = self.request_peers(torrent_file)?;
        for peer in peers {
            match self.download_from_peer(&peer) {
                Ok(_) => {}
                Err(error) => println!("Failed to connect to peer {}: {}", peer, error),
            }
        }

        Result::Ok(())
    }

    fn request_peers(&self, torrent_file: &TorrentFile) -> Result<Vec<Peer>, Box<dyn Error>> {
        let tracker_url = self.tracker_url(torrent_file)?;
        let response = reqwest::blocking::get(tracker_url)?.bytes()?;
        let tracker_response = serde_bencode::from_bytes::<TrackerResponse>(&response)?;

        let peers: Vec<Peer> = tracker_response
            .peers
            .chunks(6)
            .map(|chunk| {
                let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
                let port = u16::from_be_bytes([chunk[4], chunk[5]]);
                Peer::new(ip, port)
            })
            .collect();

        Result::Ok(peers)
    }

    fn download_from_peer(&self, peer: &Peer) -> Result<(), Box<dyn Error>> {
        let stream =
            TcpStream::connect_timeout(&(peer.ip, peer.port).into(), Duration::from_secs(3))?;
        println!("Connected to {:?}", peer);
        stream.shutdown(Shutdown::Both)?;
        Result::Ok(())
    }

    fn tracker_url(&self, torrent_file: &TorrentFile) -> Result<String, Box<dyn Error>> {
        let peer_id_encoded = urlencoding::encode_binary(&self.peer_id);
        let info_hash = torrent_file.info_hash()?;
        let info_hash_encoded = urlencoding::encode_binary(&info_hash);

        Result::Ok(format!(
            "{}?peer_id={}&info_hash={}&port={}&left={}&compact=1&uploaded=0&downloaded=0",
            torrent_file.announce,
            peer_id_encoded,
            info_hash_encoded,
            self.port.to_string(),
            torrent_file.info.length.to_string()
        ))
    }
}

impl TorrentFile {
    fn from_file(path: &Path) -> Result<Self, Box<dyn Error>> {
        let bytes = std::fs::read(path)?;
        let torrent_file = serde_bencode::from_bytes::<Self>(&bytes)?;
        Result::Ok(torrent_file)
    }

    fn info_hash(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let info_bytes = serde_bencode::to_bytes(&self.info)?;
        let mut hash = Sha1::new();
        hash.update(info_bytes);
        let hash_bytes = hash.finalize();
        Result::Ok(hash_bytes.to_vec())
    }
}

fn main() {
    let client = TorrentClient::new(6881);
    if let Some(filename) = std::env::args().skip(1).next() {
        if let Result::Ok(torrent_file) = TorrentFile::from_file(Path::new(&filename)) {
            match client.download_file(&torrent_file) {
                Ok(_) => {}
                Err(error) => println!("{:?}", error),
            }
        }
    }
}
