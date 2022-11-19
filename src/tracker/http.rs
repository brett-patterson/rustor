use std::net::Ipv4Addr;

use serde_bytes::ByteBuf;
use serde_derive::Deserialize;

use crate::{torrent::Torrent, types::PeerID};

use super::Peer;

#[derive(Debug, Deserialize)]
struct TrackerResponse {
    peers: ByteBuf,
}

pub async fn get_peers(
    peer_id: &PeerID,
    port: u16,
    torrent: &Torrent,
) -> anyhow::Result<Vec<Peer>> {
    let peer_id_encoded = urlencoding::encode_binary(peer_id);
    let info_hash_encoded = urlencoding::encode_binary(&torrent.info_hash);
    let tracker_url = format!(
        "{}?peer_id={}&info_hash={}&port={}&left={}&compact=1&uploaded=0&downloaded=0",
        torrent.announce,
        peer_id_encoded,
        info_hash_encoded,
        port.to_string(),
        torrent.length.to_string()
    );

    let response = reqwest::get(tracker_url).await?.bytes().await?;
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
