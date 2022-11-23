mod http;
mod udp;

use std::{fmt::Display, net::Ipv4Addr};

use url::Url;

use crate::{torrent::Torrent, types::PeerID};

#[derive(Debug, Clone, Copy)]
pub struct Peer {
    pub ip: Ipv4Addr,
    pub port: u16,
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

pub async fn get_peers(
    peer_id: &PeerID,
    port: u16,
    torrent: &Torrent,
) -> anyhow::Result<Vec<Peer>> {
    // TODO: Support announce list
    let url = Url::parse(&torrent.announce)?;
    match url.scheme() {
        "http" | "https" => http::get_peers(peer_id, port, torrent).await,
        "udp" => udp::get_peers(peer_id, port, torrent).await,
        scheme => Result::Err(anyhow::anyhow!(
            "Unsupported tracker URL scheme: {}",
            scheme
        )),
    }
}
