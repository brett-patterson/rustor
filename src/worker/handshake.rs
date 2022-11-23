use bytes::{Buf, BufMut, BytesMut};
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

        let pstr_len: usize = reader.read_u8().await?.try_into()?;

        let mut buf = BytesMut::zeroed(pstr_len + EXTENSIONS_LEN + INFO_HASH_LEN + PEER_ID_LEN);
        reader.read_exact(&mut buf).await?;

        let pstr = std::str::from_utf8(&buf[0..pstr_len])?;
        if pstr != PSTR {
            return Result::Err(anyhow::anyhow!(
                "Invalid protocol string, expected {} got {}",
                PSTR,
                pstr
            ));
        }
        buf.advance(pstr_len);

        // Skip extensions, we don't support any of them
        buf.advance(EXTENSIONS_LEN);

        buf.copy_to_slice(&mut h.info_hash);
        buf.copy_to_slice(&mut h.peer_id);

        Result::Ok(h)
    }

    async fn write<W: AsyncWriteExt + Unpin>(&self, writer: &mut W) -> anyhow::Result<()> {
        let mut buf =
            BytesMut::with_capacity(1 + PSTR.len() + EXTENSIONS_LEN + INFO_HASH_LEN + PEER_ID_LEN);

        // Protocol identifier length
        buf.put_u8(u8::try_from(PSTR.len())?);
        // Protocol identifier
        buf.put_slice(PSTR.as_bytes());
        // Extension bytes, all 0 since we don't support any extensions
        buf.put_bytes(0, EXTENSIONS_LEN);
        // Info hash identifying the file we want
        buf.put_slice(&self.info_hash);
        // The peer ID of our client
        buf.put_slice(&self.peer_id);

        writer.write_all(&buf).await?;

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
