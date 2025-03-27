use crate::prelude::*;
use std::net::UdpSocket;
use std::str;

const BROADCAST_ADDR: &str = "255.255.255.255:20010";
const LISTEN_ADDR: &str = "0.0.0.0:20010";

pub const MASTER: u8 = 0;
pub const MASTER_BACKUP: u8 = 1;
pub const SLAVE: u8 = 2;

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, PartialOrd)]
pub struct NetworkUnit {
    pub id: u8,
    pub role: u8,
    pub status: Status
}

impl NetworkUnit {
    pub fn new(id:u8) -> Self {
        NetworkUnit{
            id,
            role: NetworkUnit::determine_role(),
            status: NetworkUnit::fetch_status()
        }
    }
    pub fn determine_role() -> u8 {return MASTER;}
    pub fn fetch_status() -> Status {
        return Status {
            last_floor: 0,
            direction: 0,
            errors: false,
            obstructions: false,
            target_floor: None
        };
    }
    pub fn send_broadcast(&self,broadcast:Communication) {
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
        socket.set_broadcast(true).expect("Failed to enable broadcast");
        
        let message = serde_json::to_string(&broadcast).expect("Failed to serialize broadcast");
        socket.send_to(message.as_bytes(), BROADCAST_ADDR).expect("Failed to send broadcast");
    }

    pub fn receive_broadcasts(&self) -> Option<Communication> {
        let socket = UdpSocket::bind(LISTEN_ADDR).expect("Failed to bind socket");
        socket.set_read_timeout(Some(Duration::from_secs(5))).expect("Failed to set timeout");
        
        let mut buf = [0; 1024];
        match socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                let received = str::from_utf8(&buf[..size]).expect("Failed to parse received data");
                serde_json::from_str(received).ok()
            }
            Err(_) => None,
        }
    }
}