use std::net::Ipv4Addr;

use crate::{
    torrent::Torrent,
    types::{InfoHash, PeerID},
};

use super::Peer;

use bytes::{Buf, BufMut, BytesMut};
use rand::Rng;
use tokio::net::UdpSocket;
use url::Url;

const BUFFER_SIZE: usize = 1024;

const PROTOCOL_ID: i64 = 0x41727101980;
const ACTION_CONNECT: i32 = 0;
const ACTION_ANNOUNCE: i32 = 1;

fn generate_transaction_id() -> i32 {
    rand::thread_rng().gen()
}

struct UdpTrackerConnection {
    info_hash: InfoHash,
    length: u64,
    socket: UdpSocket,
    backoff: u32,
    connection_id: Option<i64>,
}

impl UdpTrackerConnection {
    async fn new(torrent: &Torrent) -> anyhow::Result<Self> {
        let url = Url::parse(&torrent.announce)?;
        let socket_addr = &*url.socket_addrs(|| None)?;
        let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;
        socket.connect(socket_addr).await?;

        Result::Ok(Self {
            info_hash: torrent.info_hash,
            length: torrent.length,
            socket,
            backoff: 0,
            connection_id: None,
        })
    }

    async fn connect(&mut self) -> anyhow::Result<()> {
        let transaction_id = generate_transaction_id();

        // TODO: timeouts and retransmission
        self.send_connect(transaction_id).await?;
        let connection_id = self.recv_connect(transaction_id).await?;
        self.connection_id = Some(connection_id);

        Result::Ok(())
    }

    async fn send_connect(&self, transaction_id: i32) -> anyhow::Result<()> {
        let mut buf = BytesMut::with_capacity(16);
        buf.put_i64(PROTOCOL_ID);
        buf.put_i32(ACTION_CONNECT);
        buf.put_i32(transaction_id);

        let sent = self.socket.send(&buf).await?;
        if sent == 16 {
            Result::Ok(())
        } else {
            Result::Err(anyhow::anyhow!(
                "Sent wrong number of bytes, expected {} got {}",
                16,
                sent
            ))
        }
    }

    async fn recv_connect(&mut self, transaction_id: i32) -> anyhow::Result<i64> {
        let mut buf = self.recv().await?;
        if buf.len() < 16 {
            return Result::Err(anyhow::anyhow!(
                "Invalid UDP connect response, expected at least 16 bytes got {}",
                buf.len()
            ));
        }

        let action = buf.get_i32();
        if action != ACTION_CONNECT {
            return Result::Err(anyhow::anyhow!(
                "Invalid UDP connect response, expected action {} got {}",
                ACTION_CONNECT,
                action
            ));
        }

        let tid = buf.get_i32();
        if tid != transaction_id {
            return Result::Err(anyhow::anyhow!(
                "Invalid UDP connect response, expected transaction ID {} got {}",
                transaction_id,
                tid
            ));
        }

        let connection_id = buf.get_i64();
        Result::Ok(connection_id)
    }

    async fn announce(&mut self, peer_id: &PeerID, port: u16) -> anyhow::Result<Vec<Peer>> {
        let connection_id = self
            .connection_id
            .ok_or_else(|| anyhow::anyhow!("Cannot announce without connection id"))?;
        let transaction_id = generate_transaction_id();

        // TODO: timeouts and retransmission
        self.send_announce(peer_id, port, connection_id, transaction_id)
            .await?;

        self.recv_announce(transaction_id).await
    }

    async fn send_announce(
        &self,
        peer_id: &PeerID,
        port: u16,
        connection_id: i64,
        transaction_id: i32,
    ) -> anyhow::Result<()> {
        let mut buf = BytesMut::with_capacity(98);
        buf.put_i64(connection_id); // connection_id
        buf.put_i32(ACTION_ANNOUNCE); // action
        buf.put_i32(transaction_id); // transaction_id
        buf.put_slice(&self.info_hash); // info_hash
        buf.put_slice(peer_id); // peer_id
        buf.put_i64(0); // downloaded
        buf.put_i64(self.length.try_into()?); // left
        buf.put_i64(0); // uploaded
        buf.put_i32(0); // event
        buf.put_i32(0); // IP address
        buf.put_i32(0); // key
        buf.put_i32(-1); // num_want
        buf.put_u16(port); // port
        self.socket.send(&buf).await?;
        Result::Ok(())
    }

    async fn recv_announce(&mut self, transaction_id: i32) -> anyhow::Result<Vec<Peer>> {
        let mut buf = self.recv().await?;
        if buf.len() < 16 {
            return Result::Err(anyhow::anyhow!(
                "Invalid UDP announce response, expected at least 16 bytes got {}",
                buf.len()
            ));
        }

        let action = buf.get_i32();
        if action != ACTION_ANNOUNCE {
            return Result::Err(anyhow::anyhow!(
                "Invalid UDP announce response, expected action {} got {}",
                ACTION_ANNOUNCE,
                action
            ));
        }

        let tid = buf.get_i32();
        if tid != transaction_id {
            return Result::Err(anyhow::anyhow!(
                "Invalid UDP announce response, expected transaction ID {} got {}",
                transaction_id,
                tid
            ));
        }

        let _interval = buf.get_i32();
        let _leechers = buf.get_i32();
        let _seeders = buf.get_i32();

        let mut peers = Vec::<Peer>::new();
        while buf.remaining() >= 6 {
            let ip = Ipv4Addr::new(buf.get_u8(), buf.get_u8(), buf.get_u8(), buf.get_u8());
            let port = buf.get_u16();
            peers.push(Peer { ip, port });
        }

        Result::Ok(peers)
    }

    async fn recv(&mut self) -> anyhow::Result<BytesMut> {
        let mut buf = BytesMut::zeroed(BUFFER_SIZE);
        let n = self.socket.recv(&mut buf).await?;
        buf.truncate(n);
        Result::Ok(buf)
    }

    // TODO: To be used once timeouts and retransmission is implemented
    #[allow(dead_code)]
    fn timeout(&self) -> u64 {
        15 * 2u64.pow(self.backoff)
    }
}

pub async fn get_peers(
    peer_id: &PeerID,
    port: u16,
    torrent: &Torrent,
) -> anyhow::Result<Vec<Peer>> {
    let mut conn = UdpTrackerConnection::new(torrent).await?;
    conn.connect().await?;
    conn.announce(peer_id, port).await
}
