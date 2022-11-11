mod handshake;
mod message;

use std::time::Duration;

use anyhow::Context;
use async_channel::{Receiver, Sender};
use sha1::{Digest, Sha1};
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedSender;

use self::handshake::handshake;
use self::message::{Bitfield, Message};
use crate::types::PieceHash;
use crate::{
    tracker::Peer,
    types::{InfoHash, PeerID},
};

// The largest number of bytes a request can ask for
const MAX_BLOCK_SIZE: u32 = 16384;

// The max number of unfulfilled requests a worker can have in flight at one time
const MAX_BACKLOG: u32 = 5;

#[derive(Debug)]
pub struct PieceInfo {
    index: u32,
    hash: PieceHash,
    length: u32,
}

impl PieceInfo {
    pub fn new(index: u32, hash: PieceHash, length: u32) -> Self {
        Self {
            index,
            hash,
            length,
        }
    }
}

pub type DownloadChannel = (Sender<PieceInfo>, Receiver<PieceInfo>);

#[derive(Debug)]
pub struct PieceResult {
    pub index: u32,
    pub buf: Vec<u8>,
}

impl PieceResult {
    pub fn new(index: u32, buf: Vec<u8>) -> Self {
        Self { index, buf }
    }
}

struct PieceProgress {
    buf: Vec<u8>,
    downloaded: u32,
    requested: u32,
    backlog: u32,
}

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
            .await
            .context("Timed out waiting for bitfield message")??;

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

    pub async fn start(
        &mut self,
        (download_sender, download_receiver): DownloadChannel,
        result_sender: UnboundedSender<PieceResult>,
    ) -> anyhow::Result<()> {
        Message::Unchoke.write(&mut self.stream).await?;
        Message::Interested.write(&mut self.stream).await?;

        while let Ok(piece_info) = download_receiver.recv().await {
            if !self.bitfield.has(piece_info.index) {
                // This peer doesn't have this piece, send it back for another worker to pick up
                download_sender.send(piece_info).await?;
                continue;
            }

            match self.download_piece(&piece_info).await {
                Ok(piece) => {
                    // TODO: check piece integrity
                    let mut sha1 = Sha1::new();
                    sha1.update(&piece);
                    let hash: PieceHash = sha1.finalize().try_into()?;
                    if hash != piece_info.hash {
                        return Result::Err(anyhow::anyhow!(
                            "Failed integrity check for piece {}",
                            piece_info.index
                        ));
                    }

                    Message::Have(piece_info.index)
                        .write(&mut self.stream)
                        .await?;

                    result_sender.send(PieceResult::new(piece_info.index, piece))?;
                }
                Err(error) => {
                    // If we failed to download a piece, put the piece info back into the queue and
                    // disconnect from this peer
                    download_sender.send(piece_info).await?;
                    return Result::Err(error);
                }
            }
        }

        Result::Ok(())
    }

    async fn download_piece(&mut self, piece_info: &PieceInfo) -> anyhow::Result<Vec<u8>> {
        let mut progress = PieceProgress {
            buf: vec![0u8; piece_info.length as usize],
            downloaded: 0,
            requested: 0,
            backlog: 0,
        };

        while progress.downloaded < piece_info.length {
            if !self.choked {
                while progress.backlog < MAX_BACKLOG && progress.requested < piece_info.length {
                    let block_size =
                        u32::min(MAX_BLOCK_SIZE, piece_info.length - progress.requested);

                    Message::Request(piece_info.index, progress.requested, block_size)
                        .write(&mut self.stream)
                        .await?;
                    progress.backlog += 1;
                    progress.requested += block_size;
                }
            }

            let msg =
                tokio::time::timeout(Duration::from_secs(30), Message::read(&mut self.stream))
                    .await??;

            match msg {
                Message::Choke => {
                    self.choked = true;
                }
                Message::Unchoke => {
                    self.choked = false;
                }
                Message::Have(index) => {
                    self.bitfield.set(index);
                }
                Message::Piece(index, begin, block) => {
                    if index != piece_info.index {
                        println!(
                            "Received incorrect piece index: expected {}, got {}",
                            piece_info.index, index
                        );
                        continue;
                    }

                    let b = begin as usize;
                    if b >= progress.buf.len() {
                        return Result::Err(anyhow::anyhow!(
                            "Block begin offset {} too high for piece buffer size {}",
                            begin,
                            progress.buf.len()
                        ));
                    }

                    if b + block.len() > progress.buf.len() {
                        return Result::Err(anyhow::anyhow!(
                            "Block buffer of size {} at begin {} too large to put into piece buffer of size {}",
                            block.len(),
                            b,
                            progress.buf.len()
                        ));
                    }

                    progress.buf[b..b + block.len()].copy_from_slice(&block);
                    progress.downloaded += block.len() as u32;
                    progress.backlog -= 1;
                }
                _ => {}
            }
        }

        Result::Ok(progress.buf)
    }
}
