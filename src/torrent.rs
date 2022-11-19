use std::path::PathBuf;

use crate::{
    torrent_file::{TorrentMetaInfo, TorrentMetaInfoInfoFile},
    types::{InfoHash, PieceHash},
};

pub struct Torrent {
    pub name: String,
    pub announce: String,
    pub length: u64,
    pub info_hash: InfoHash,
    pub piece_length: u64,
    pub piece_hashes: Vec<PieceHash>,
    pub files: Vec<TorrentFile>,
}

pub struct TorrentFile {
    pub length: u64,
    pub path: PathBuf,
}

impl TryFrom<TorrentMetaInfo> for Torrent {
    type Error = anyhow::Error;

    fn try_from(i: TorrentMetaInfo) -> Result<Self, Self::Error> {
        let info_hash = i.info_hash()?;
        let piece_hashes = i.piece_hashes()?;

        if let Some(length) = i.info.length {
            // Single file case
            let path = PathBuf::from(&i.info.name);
            Result::Ok(Self {
                name: i.info.name,
                announce: i.announce,
                length,
                info_hash,
                piece_length: i.info.piece_length,
                piece_hashes,
                files: vec![TorrentFile { length, path }],
            })
        } else if let Some(files) = i.info.files {
            // Multi-file case
            Result::Ok(Self {
                name: i.info.name,
                announce: i.announce,
                length: files.iter().map(|f| f.length).sum(),
                info_hash,
                piece_length: i.info.piece_length,
                piece_hashes,
                files: files.iter().map(TorrentFile::from).collect(),
            })
        } else {
            Result::Err(anyhow::anyhow!(
                "Invalid torrent meta info, expected length or files"
            ))
        }
    }
}

impl From<&TorrentMetaInfoInfoFile> for TorrentFile {
    fn from(f: &TorrentMetaInfoInfoFile) -> Self {
        let mut path = PathBuf::new();
        for p in &f.path {
            path.push(p);
        }

        Self {
            length: f.length,
            path,
        }
    }
}
