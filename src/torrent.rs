use crate::{torrent_file::TorrentFile, types::InfoHash};

pub struct Torrent {
    pub name: String,
    pub announce: String,
    pub length: i64,
    pub info_hash: InfoHash,
    pub piece_length: i64,
}

impl TryFrom<TorrentFile> for Torrent {
    type Error = anyhow::Error;

    fn try_from(f: TorrentFile) -> Result<Self, Self::Error> {
        let info_hash = f.info_hash()?;
        Result::Ok(Self {
            name: f.info.name,
            announce: f.announce,
            length: f.info.length,
            info_hash,
            piece_length: f.info.piece_length,
        })
    }
}
