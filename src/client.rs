use std::io::SeekFrom;

use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::torrent::Torrent;
use crate::torrent_file::TorrentFile;
use crate::tracker::get_peers;
use crate::types::{PeerID, PEER_ID_LEN};
use crate::worker::{PieceInfo, PieceResult, TorrentDownloadWorker};

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

    pub async fn download_file(&self, torrent_file: TorrentFile) -> anyhow::Result<()> {
        let torrent = Torrent::try_from(torrent_file)?;
        let peers = get_peers(&self.peer_id, self.port, &torrent).await?;

        let mut file = File::create(torrent.name).await?;

        let (download_sender, download_receiver) = async_channel::unbounded::<PieceInfo>();
        let (result_sender, mut result_receiver) = mpsc::unbounded_channel::<PieceResult>();
        let progress = ProgressBar::new(torrent.length).with_style(ProgressStyle::with_template(
            "[{percent}%] {wide_bar} {eta} ({bytes_per_sec})",
        )?);

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
        for i in 0..torrent.piece_hashes.len() {
            let begin = i as u64 * torrent.piece_length;
            let end = u64::min((i + 1) as u64 * torrent.piece_length, torrent.length);
            let length = end - begin;
            let piece_info = PieceInfo::new(i as u32, torrent.piece_hashes[i], length as u32);
            download_sender.send(piece_info).await?;
        }

        let mut bytes_written = 0u64;
        while let Some(piece_result) = result_receiver.recv().await {
            let begin = piece_result.index as u64 * torrent.piece_length;
            file.seek(SeekFrom::Start(begin)).await?;
            file.write_all(&piece_result.buf).await?;
            bytes_written += piece_result.buf.len() as u64;
            progress.set_position(bytes_written);
            // println!(
            //     "[{}% ({} / {})] Wrote piece {}",
            //     bytes_written as f64 / torrent.length as f64 * 100f64,
            //     bytes_written,
            //     torrent.length,
            //     piece_result.index
            // );

            if bytes_written == torrent.length {
                break;
            }
        }

        progress.finish();
        println!("Shutting down");
        download_receiver.close();

        for task in tasks {
            match task.await {
                Ok(task_result) => match task_result {
                    Ok(_) => {}
                    Err(error) => {
                        println!("{}", error)
                    }
                },
                Err(error) => {
                    println!("Failed to join task: {}", error)
                }
            }
        }

        Result::Ok(())
    }
}
