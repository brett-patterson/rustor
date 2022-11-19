use std::path::{Path, PathBuf};

use tokio::fs::File;

use crate::torrent::{Torrent, TorrentFile};

pub struct TorrentWriter {
    files: Vec<TorrentWriterFileHandle>,
    bytes_written: u64,
}

struct TorrentWriterFileHandle {
    file: File,
    length: u64,
}

impl TorrentWriter {
    pub async fn from_torrent(torrent: &Torrent) -> anyhow::Result<Self> {
        let files: Vec<TorrentWriterFileHandle> = if torrent.files.len() == 1 {
            // For single files, we can just create the single file to write to
            let f = &torrent.files[0];
            vec![TorrentWriterFileHandle {
                file: File::create(&f.path).await?,
                length: f.length,
            }]
        } else {
            // For multiple files, we need to set up the directory structure for the files
            let base_dir_path = PathBuf::try_from(&torrent.name)?;
            tokio::fs::create_dir(&base_dir_path).await?;

            let mut result: Vec<TorrentWriterFileHandle> = Vec::with_capacity(torrent.files.len());
            for f in torrent.files.iter() {
                // Create intermediate directories if needed
                if let Some(parent) = f.path.parent() {
                    tokio::fs::create_dir_all(base_dir_path.join(parent)).await?;
                }

                let file = File::create(base_dir_path.join(&f.path)).await?;
                result.push(TorrentWriterFileHandle {
                    file,
                    length: f.length,
                });
            }

            result
        };

        Result::Ok(Self {
            files,
            bytes_written: 0,
        })
    }
}
