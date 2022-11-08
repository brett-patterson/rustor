use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::types::{InfoHash, PeerID, INFO_HASH_LEN, PEER_ID_LEN};

const PSTR: &str = "BitTorrent protocol";
const EXTENSIONS_LEN: usize = 8;

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

pub async fn handshake(
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
