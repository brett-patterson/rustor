use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MESSAGE_ID_CHOKE: u8 = 0;
const MESSAGE_ID_UNCHOKE: u8 = 1;
const MESSAGE_ID_INTERESTED: u8 = 2;
const MESSAGE_ID_NOT_INTERESTED: u8 = 3;
const MESSAGE_ID_HAVE: u8 = 4;
const MESSAGE_ID_BITFIELD: u8 = 5;
const MESSAGE_ID_REQUEST: u8 = 6;
const MESSAGE_ID_PIECE: u8 = 7;
const MESSAGE_ID_CANCEL: u8 = 8;

#[derive(Debug)]
pub struct Bitfield(Vec<u8>);

impl Bitfield {
    pub fn has(&self, index: u32) -> bool {
        let byte_index = index / 8;
        let bit_offset = index % 8;
        self.0[byte_index as usize] >> (7 - bit_offset) & 1 != 0
    }

    pub fn set(&mut self, index: u32) {
        let byte_index = index / 8;
        let bit_offset = index % 8;
        self.0[byte_index as usize] |= 1 << (7 - bit_offset);
    }
}

#[derive(Debug)]
pub enum Message {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Bitfield(Bitfield),
    Have(u32),
    Request(u32, u32, u32),
    Cancel(u32, u32, u32),
    Piece(u32, u32, Vec<u8>),
}

impl Message {
    pub async fn read<R: AsyncReadExt + Unpin>(reader: &mut R) -> anyhow::Result<Self> {
        let len = reader.read_u32().await?;
        if len == 0 {
            return Result::Ok(Self::KeepAlive);
        }

        let mut payload = vec![0u8; len as usize];
        reader.read_exact(&mut payload).await?;

        let id = payload[0];

        match id {
            MESSAGE_ID_CHOKE => Result::Ok(Self::Choke),
            MESSAGE_ID_UNCHOKE => Result::Ok(Self::Unchoke),
            MESSAGE_ID_INTERESTED => Result::Ok(Self::Interested),
            MESSAGE_ID_NOT_INTERESTED => Result::Ok(Self::NotInterested),
            MESSAGE_ID_BITFIELD => {
                let mut bitfield = vec![0u8; payload.len() - 1];
                bitfield[..].copy_from_slice(&payload[1..]);
                Result::Ok(Self::Bitfield(Bitfield(bitfield)))
            }
            MESSAGE_ID_HAVE => {
                let index = u32::from_be_bytes(payload[1..5].try_into()?);
                Result::Ok(Self::Have(index))
            }
            MESSAGE_ID_REQUEST => {
                let index = u32::from_be_bytes(payload[1..5].try_into()?);
                let begin = u32::from_be_bytes(payload[5..9].try_into()?);
                let length = u32::from_be_bytes(payload[9..13].try_into()?);
                Result::Ok(Self::Request(index, begin, length))
            }
            MESSAGE_ID_CANCEL => {
                let index = u32::from_be_bytes(payload[1..5].try_into()?);
                let begin = u32::from_be_bytes(payload[5..9].try_into()?);
                let length = u32::from_be_bytes(payload[9..13].try_into()?);
                Result::Ok(Self::Cancel(index, begin, length))
            }
            MESSAGE_ID_PIECE => {
                let index = u32::from_be_bytes(payload[1..5].try_into()?);
                let begin = u32::from_be_bytes(payload[5..9].try_into()?);
                let mut block = vec![0u8; payload.len() - 1 - 8];
                block[..].copy_from_slice(&payload[9..]);
                Result::Ok(Self::Piece(index, begin, block))
            }
            _ => Result::Err(anyhow::anyhow!("Unknown message ID: {}", id)),
        }
    }

    pub async fn write<W: AsyncWriteExt + Unpin>(&self, writer: &mut W) -> anyhow::Result<()> {
        let buf: Vec<u8> = match self {
            Self::KeepAlive => {
                vec![0u8; 1]
            }
            Self::Choke => {
                let mut buf = vec![0u8; 4 + 1];
                buf[0..4].copy_from_slice(&u32::to_be_bytes(1));
                buf[4] = MESSAGE_ID_CHOKE;
                buf
            }
            Self::Unchoke => {
                let mut buf = vec![0u8; 4 + 1];
                buf[0..4].copy_from_slice(&u32::to_be_bytes(1));
                buf[4] = MESSAGE_ID_UNCHOKE;
                buf
            }
            Self::Interested => {
                let mut buf = vec![0u8; 4 + 1];
                buf[0..4].copy_from_slice(&u32::to_be_bytes(1));
                buf[4] = MESSAGE_ID_INTERESTED;
                buf
            }
            Self::NotInterested => {
                let mut buf = vec![0u8; 4 + 1];
                buf[0..4].copy_from_slice(&u32::to_be_bytes(1));
                buf[4] = MESSAGE_ID_NOT_INTERESTED;
                buf
            }
            Self::Bitfield(bitfield) => {
                let mut buf = vec![0u8; 4 + 1 + bitfield.0.len()];
                buf[0..4].copy_from_slice(&u32::to_be_bytes(1 + bitfield.0.len() as u32));
                buf[4] = MESSAGE_ID_BITFIELD;
                buf[5..].copy_from_slice(&bitfield.0);
                buf
            }
            Self::Have(index) => {
                let mut buf = vec![0u8; 4 + 1 + 4];
                buf[0..4].copy_from_slice(&u32::to_be_bytes(1 + 4));
                buf[4] = MESSAGE_ID_HAVE;
                buf[5..].copy_from_slice(&index.to_be_bytes());
                buf
            }
            Self::Request(index, begin, length) => {
                let mut buf = vec![0u8; 4 + 1 + 4 + 4 + 4];
                buf[0..4].copy_from_slice(&u32::to_be_bytes(1 + 4 + 4 + 4));
                buf[4] = MESSAGE_ID_REQUEST;
                buf[5..9].copy_from_slice(&index.to_be_bytes());
                buf[9..13].copy_from_slice(&begin.to_be_bytes());
                buf[13..17].copy_from_slice(&length.to_be_bytes());
                buf
            }
            Self::Cancel(index, begin, length) => {
                let mut buf = vec![0u8; 4 + 1 + 4 + 4 + 4];
                buf[0..4].copy_from_slice(&u32::to_be_bytes(1 + 4 + 4 + 4));
                buf[4] = MESSAGE_ID_CANCEL;
                buf[5..9].copy_from_slice(&index.to_be_bytes());
                buf[9..13].copy_from_slice(&begin.to_be_bytes());
                buf[13..17].copy_from_slice(&length.to_be_bytes());
                buf
            }
            Self::Piece(index, begin, block) => {
                let mut buf = vec![0u8; 4 + 1 + 4 + 4 + block.len()];
                buf[0..4].copy_from_slice(&u32::to_be_bytes(1 + 4 + 4 + block.len() as u32));
                buf[4] = MESSAGE_ID_PIECE;
                buf[5..9].copy_from_slice(&index.to_be_bytes());
                buf[9..13].copy_from_slice(&begin.to_be_bytes());
                buf[13..].copy_from_slice(&block);
                buf
            }
        };

        writer.write_all(&buf).await?;
        Result::Ok(())
    }
}
