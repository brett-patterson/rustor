use rand::Rng;
use tokio::task::JoinHandle;

use crate::torrent::Torrent;
use crate::torrent_file::TorrentFile;
use crate::tracker::get_peers;
use crate::types::{PeerID, PEER_ID_LEN};
use crate::worker::TorrentDownloadWorker;

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
        let tasks: Vec<JoinHandle<anyhow::Result<()>>> = peers
            .into_iter()
            .map(|peer| {
                let peer_id = self.peer_id.clone();
                let info_hash = torrent.info_hash.clone();
                tokio::spawn(async move {
                    let worker =
                        TorrentDownloadWorker::connect(&info_hash, &peer_id, &peer).await?;
                    println!("Worker: {:?}", worker);
                    Result::Ok(())
                })
            })
            .collect();

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
