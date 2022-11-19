use std::path::Path;

use serde_bytes::ByteBuf;
use serde_derive::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::types::{InfoHash, PieceHash, PIECE_HASH_LEN};

#[derive(Debug, Deserialize)]
pub struct TorrentMetaInfo {
    pub announce: String,
    pub info: TorrentMetaInfoInfo,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TorrentMetaInfoInfo {
    pub name: String,
    pub length: Option<u64>,
    pub files: Option<Vec<TorrentMetaInfoInfoFile>>,
    pub pieces: ByteBuf,
    #[serde(rename = "piece length")]
    pub piece_length: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TorrentMetaInfoInfoFile {
    pub length: u64,
    pub path: Vec<String>,
}

impl TorrentMetaInfo {
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        let torrent_file = serde_bencode::from_bytes::<Self>(&bytes)?;
        Result::Ok(torrent_file)
    }

    pub fn info_hash(&self) -> anyhow::Result<InfoHash> {
        let info_bytes = serde_bencode::to_bytes(&self.info)?;
        let mut hash = Sha1::new();
        hash.update(info_bytes);
        Result::Ok(hash.finalize().try_into()?)
    }

    pub fn piece_hashes(&self) -> anyhow::Result<Vec<PieceHash>> {
        let n = self.info.pieces.len() / PIECE_HASH_LEN;
        let mut hashes = vec![[0u8; PIECE_HASH_LEN]; n];

        for i in 0..n {
            hashes[i]
                .copy_from_slice(&self.info.pieces[i * PIECE_HASH_LEN..(i + 1) * PIECE_HASH_LEN]);
        }

        Result::Ok(hashes)
    }
}
