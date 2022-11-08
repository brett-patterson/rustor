mod handshake;
mod message;

use std::time::Duration;

use anyhow::Context;
use tokio::net::TcpStream;

use self::handshake::handshake;
use self::message::{Bitfield, Message};
use crate::{
    tracker::Peer,
    types::{InfoHash, PeerID},
};

#[derive(Debug)]
pub struct TorrentDownloadWorker {
    stream: TcpStream,
    choked: bool,
    bitfield: Bitfield,
}

impl TorrentDownloadWorker {
    pub async fn connect(
        info_hash: &InfoHash,
        peer_id: &PeerID,
        peer: &Peer,
    ) -> anyhow::Result<Self> {
        let mut stream = tokio::time::timeout(
            Duration::from_secs(3),
            TcpStream::connect((peer.ip, peer.port)),
        )
        .await
        .context("Connection timeout")??;

        handshake(&mut stream, info_hash, peer_id).await?;

        println!("Connected to {}", peer);

        let msg = tokio::time::timeout(Duration::from_secs(5), Message::read(&mut stream))
            .await?
            .context("Timed out waiting for bitfield message")?;

        match msg {
            Message::Bitfield(bitfield) => Result::Ok(Self {
                stream,
                choked: true,
                bitfield,
            }),
            msg => Result::Err(anyhow::anyhow!(
                "Invalid message, expected bitfield, got {:?}",
                msg
            )),
        }
    }
}
