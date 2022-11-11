use crate::{
    torrent_file::TorrentFile,
    types::{InfoHash, PieceHash},
};

pub struct Torrent {
    pub name: String,
    pub announce: String,
    pub length: u64,
    pub info_hash: InfoHash,
    pub piece_length: u64,
    pub piece_hashes: Vec<PieceHash>,
}

impl TryFrom<TorrentFile> for Torrent {
    type Error = anyhow::Error;

    fn try_from(f: TorrentFile) -> Result<Self, Self::Error> {
        let info_hash = f.info_hash()?;
        let piece_hashes = f.piece_hashes()?;

        Result::Ok(Self {
            name: f.info.name,
            announce: f.announce,
            length: f.info.length,
            info_hash,
            piece_length: f.info.piece_length,
            piece_hashes,
        })
    }
}
