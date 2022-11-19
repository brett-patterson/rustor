use rand::Rng;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::torrent::Torrent;
use crate::torrent_file::TorrentMetaInfo;
use crate::tracker::get_peers;
use crate::types::{PeerID, PEER_ID_LEN};
use crate::worker::{PieceInfo, PieceResult, TorrentDownloadWorker};
use crate::writer::TorrentWriter;

pub struct TorrentClient {
    peer_id: PeerID,
    port: u16,
}

impl TorrentClient {
    pub fn new(port: u16) -> Self {
        let mut peer_id = [0u8; PEER_ID_LEN];
        rand::thread_rng().fill(&mut peer_id);
        Self { peer_id, port }
    }

    pub async fn download_file(&self, torrent_file: TorrentMetaInfo) -> anyhow::Result<()> {
        let torrent = Torrent::try_from(torrent_file)?;
        let peers = get_peers(&self.peer_id, self.port, &torrent).await?;

        let (download_sender, download_receiver) = async_channel::unbounded::<PieceInfo>();
        let (result_sender, mut result_receiver) = mpsc::unbounded_channel::<PieceResult>();

        // Spawn a worker to connect to each peer
        let tasks: Vec<JoinHandle<anyhow::Result<()>>> = peers
            .into_iter()
            .map(|peer| {
                let peer_id = self.peer_id.clone();
                let info_hash = torrent.info_hash.clone();
                let channel = (download_sender.clone(), download_receiver.clone());
                let results = result_sender.clone();
                tokio::spawn(async move {
                    let mut worker =
                        TorrentDownloadWorker::connect(&info_hash, &peer_id, &peer).await?;
                    worker.start(channel, results).await?;
                    Result::Ok(())
                })
            })
            .collect();

        // Send out each piece of the file to workers
        // TODO: Better piece picking algorithm: https://luminarys.com/posts/writing-a-bittorrent-client.html
        for i in 0..torrent.piece_hashes.len() {
            let begin = i as u64 * torrent.piece_length;
            let end = u64::min((i + 1) as u64 * torrent.piece_length, torrent.length);
            let length = end - begin;
            let piece_info = PieceInfo::new(i as u32, torrent.piece_hashes[i], length as u32);
            download_sender.send(piece_info).await?;
        }

        let mut writer = TorrentWriter::from_torrent(&torrent).await?;

        let mut bytes_written = 0u64;
        while let Some(piece_result) = result_receiver.recv().await {
            let offset = piece_result.index as u64 * torrent.piece_length;
            writer.write(offset, &piece_result.buf).await?;

            bytes_written += piece_result.buf.len() as u64;

            if bytes_written == torrent.length {
                break;
            }
        }

        info!("Download finished for {}", &torrent.name);
        download_receiver.close();

        for task in tasks {
            match task.await {
                Ok(task_result) => match task_result {
                    Ok(_) => {}
                    Err(error) => {
                        warn!("Error in worker: {}", error);
                    }
                },
                Err(error) => {
                    error!("Failed to join task: {}", error);
                }
            }
        }

        Result::Ok(())
    }
}
