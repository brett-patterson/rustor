use std::time::Duration;

use anyhow::Context;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::{
    message::{Bitfield, Message},
    tracker::Peer,
    types::{InfoHash, PeerID, INFO_HASH_LEN, PEER_ID_LEN},
};

const PSTR: &str = "BitTorrent protocol";
const EXTENSIONS_LEN: usize = 8;

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

        match Message::read(&mut stream).await? {
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

struct Handshake {
    info_hash: InfoHash,
    peer_id: PeerID,
}

impl Handshake {
    fn new(info_hash: InfoHash, peer_id: PeerID) -> Self {
        Self { info_hash, peer_id }
    }

    async fn read<R: AsyncReadExt + Unpin>(reader: &mut R) -> anyhow::Result<Self> {
        let mut h = Handshake::new([0u8; INFO_HASH_LEN], [0u8; PEER_ID_LEN]);

        let pstr_len = reader.read_u8().await?;
        let mut pstr = vec![0u8; pstr_len.into()];
        reader.read_exact(&mut pstr).await?;

        let mut extensions = [0u8; EXTENSIONS_LEN];
        reader.read_exact(&mut extensions).await?;

        reader.read_exact(&mut h.info_hash).await?;

        reader.read_exact(&mut h.peer_id).await?;

        Result::Ok(h)
    }

    async fn write<W: AsyncWriteExt + Unpin>(&self, writer: &mut W) -> anyhow::Result<()> {
        // Protocol identifier length
        writer.write_u8(u8::try_from(PSTR.len())?).await?;
        // Protocol identifier
        writer.write_all(PSTR.as_bytes()).await?;
        // Extension bytes, all 0 since we don't support any extensions
        let extensions = [0u8; EXTENSIONS_LEN];
        writer.write_all(&extensions).await?;
        // Info hash identifying the file we want
        writer.write_all(&self.info_hash).await?;
        // The peer ID of our client
        writer.write_all(&self.peer_id).await?;

        Result::Ok(())
    }
}

async fn handshake(
    stream: &mut TcpStream,
    info_hash: &InfoHash,
    peer_id: &PeerID,
) -> anyhow::Result<()> {
    let send = Handshake::new(info_hash.clone(), peer_id.clone());
    send.write(stream).await?;
    let recv = Handshake::read(stream).await?;

    if send.info_hash == recv.info_hash {
        Result::Ok(())
    } else {
        Result::Err(anyhow::anyhow!("Mismatched info hashes"))
    }
}
