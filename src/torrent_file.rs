use std::path::Path;

use serde_bytes::ByteBuf;
use serde_derive::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::types::InfoHash;

#[derive(Debug, Deserialize)]
pub struct TorrentFile {
    pub announce: String,
    pub info: TorrentFileInfo,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TorrentFileInfo {
    pub name: String,
    pub length: i64,
    pub pieces: ByteBuf,
    #[serde(rename = "piece length")]
    pub piece_length: i64,
}

impl TorrentFile {
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
}
