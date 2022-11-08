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
    pub fn has_piece(&self, piece_index: usize) -> bool {
        let byte_index = piece_index / 8;
        let bit_offset = piece_index % 8;
        self.0[byte_index] >> (7 - bit_offset) & 1 != 0
    }

    pub fn set_piece(&mut self, piece_index: usize) {
        let byte_index = piece_index / 8;
        let bit_offset = piece_index % 8;
        self.0[byte_index] |= 1 << (7 - bit_offset);
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

        let id = reader.read_u8().await?;
        let payload_len = usize::try_from(len)? - 1;

        match id {
            MESSAGE_ID_CHOKE => Result::Ok(Self::Choke),
            MESSAGE_ID_UNCHOKE => Result::Ok(Self::Unchoke),
            MESSAGE_ID_INTERESTED => Result::Ok(Self::Interested),
            MESSAGE_ID_NOT_INTERESTED => Result::Ok(Self::NotInterested),
            MESSAGE_ID_BITFIELD => {
                let mut bitfield = vec![0u8; payload_len];
                reader.read_exact(&mut bitfield).await?;
                Result::Ok(Self::Bitfield(Bitfield(bitfield)))
            }
            MESSAGE_ID_HAVE => {
                let index = reader.read_u32().await?;
                Result::Ok(Self::Have(index))
            }
            MESSAGE_ID_REQUEST => {
                let index = reader.read_u32().await?;
                let begin = reader.read_u32().await?;
                let length = reader.read_u32().await?;
                Result::Ok(Self::Request(index, begin, length))
            }
            MESSAGE_ID_CANCEL => {
                let index = reader.read_u32().await?;
                let begin = reader.read_u32().await?;
                let length = reader.read_u32().await?;
                Result::Ok(Self::Cancel(index, begin, length))
            }
            MESSAGE_ID_PIECE => {
                let index = reader.read_u32().await?;
                let begin = reader.read_u32().await?;
                let mut piece = vec![0u8; payload_len - 8];
                reader.read_exact(&mut piece).await?;
                Result::Ok(Self::Piece(index, begin, piece))
            }
            _ => Result::Err(anyhow::anyhow!("Unknown message ID: {}", id)),
        }
    }

    pub async fn write<W: AsyncWriteExt + Unpin>(&self, writer: &mut W) -> anyhow::Result<()> {
        match self {
            Self::KeepAlive => {
                writer.write_u32(0).await?;
                Result::Ok(())
            }
            Self::Choke => {
                writer.write_u32(1).await?;
                writer.write_u8(MESSAGE_ID_CHOKE).await?;
                Result::Ok(())
            }
            Self::Unchoke => {
                writer.write_u32(1).await?;
                writer.write_u8(MESSAGE_ID_UNCHOKE).await?;
                Result::Ok(())
            }
            Self::Interested => {
                writer.write_u32(1).await?;
                writer.write_u8(MESSAGE_ID_INTERESTED).await?;
                Result::Ok(())
            }
            Self::NotInterested => {
                writer.write_u32(1).await?;
                writer.write_u8(MESSAGE_ID_NOT_INTERESTED).await?;
                Result::Ok(())
            }
            Self::Bitfield(bitfield) => {
                writer.write_u32((1 + bitfield.0.len()).try_into()?).await?;
                writer.write_u8(MESSAGE_ID_BITFIELD).await?;
                writer.write_all(&bitfield.0).await?;
                Result::Ok(())
            }
            Self::Have(index) => {
                writer.write_u32(1 + 4).await?;
                writer.write_u8(MESSAGE_ID_HAVE).await?;
                writer.write_u32(*index).await?;
                Result::Ok(())
            }
            Self::Request(index, begin, length) => {
                writer.write_u32(1 + 4 + 4 + 4).await?;
                writer.write_u8(MESSAGE_ID_REQUEST).await?;
                writer.write_u32(*index).await?;
                writer.write_u32(*begin).await?;
                writer.write_u32(*length).await?;
                Result::Ok(())
            }
            Self::Cancel(index, begin, length) => {
                writer.write_u32(1 + 4 + 4 + 4).await?;
                writer.write_u8(MESSAGE_ID_CANCEL).await?;
                writer.write_u32(*index).await?;
                writer.write_u32(*begin).await?;
                writer.write_u32(*length).await?;
                Result::Ok(())
            }
            Self::Piece(index, begin, piece) => {
                writer
                    .write_u32((1 + 4 + 4 + piece.len()).try_into()?)
                    .await?;
                writer.write_u8(MESSAGE_ID_CANCEL).await?;
                writer.write_u32(*index).await?;
                writer.write_u32(*begin).await?;
                writer.write_all(piece).await?;
                Result::Ok(())
            }
        }
    }
}
