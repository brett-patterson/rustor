use std::io::SeekFrom;
use std::path::PathBuf;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

use crate::torrent::{Torrent, TorrentFile};

pub struct TorrentWriter {
    files: Vec<TorrentWriterFileHandle>,
}

struct TorrentWriterFileHandle {
    file: File,
    length: u64,
    written: u64,
    progress: ProgressBar,
}

impl TorrentWriter {
    pub async fn from_torrent(torrent: &Torrent) -> anyhow::Result<Self> {
        let total_progress = MultiProgress::new();
        let files: Vec<TorrentWriterFileHandle> = if torrent.files.len() == 1 {
            // For single files, we can just create the single file to write to
            let f = &torrent.files[0];
            let progress = total_progress.add(Self::progress_bar(f)?);
            vec![TorrentWriterFileHandle {
                file: File::create(&f.path).await?,
                length: f.length,
                written: 0,
                progress,
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
                let progress = total_progress.add(Self::progress_bar(f)?);
                result.push(TorrentWriterFileHandle {
                    file,
                    length: f.length,
                    written: 0,
                    progress,
                });
            }

            result
        };

        Result::Ok(Self { files })
    }

    pub async fn write(&mut self, offset: u64, buf: &[u8]) -> anyhow::Result<()> {
        let mut buf_offset: usize = 0;
        let buf_length = buf.len();
        let mut current_handle_offset: u64 = 0;

        for h in &mut self.files {
            if offset < current_handle_offset + h.length {
                let file_offset = offset - current_handle_offset;
                let write_length =
                    usize::min((h.length - file_offset) as usize, buf_length - buf_offset);
                h.file.seek(SeekFrom::Start(file_offset)).await?;
                h.file
                    .write_all(&buf[buf_offset..buf_offset + write_length])
                    .await?;
                buf_offset += write_length;

                h.progress.inc(write_length as u64);
                h.written += write_length as u64;

                if h.written == h.length {
                    h.progress.finish();
                }
            }

            current_handle_offset += h.length;

            if buf_offset == buf_length {
                break;
            }
        }

        Result::Ok(())
    }

    fn progress_bar(file: &TorrentFile) -> anyhow::Result<ProgressBar> {
        let message = file
            .path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Could not convert file path to string"))?
            .to_owned();
        let style =
            ProgressStyle::with_template("[{percent}%] {msg} {wide_bar} {eta} ({bytes_per_sec})")?;

        let progress_bar = ProgressBar::new(file.length)
            .with_message(message)
            .with_style(style);

        Result::Ok(progress_bar)
    }
}
