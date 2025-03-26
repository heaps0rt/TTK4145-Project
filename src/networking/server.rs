use crate::prelude::*;
use std::net::UdpSocket;
use std::str;
use serde_json::from_str;

const BROADCAST_ADDR: &str = "255.255.255.255:34254";

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, PartialOrd, Serialize, Deserialize)]
pub struct Broadcast {
    pub id: u8,
    pub role: u8,
    pub status: Status
}

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
    pub fn determine_role() -> u8 {return 0;}
    pub fn fetch_status() -> Status {
        return Status {
            last_floor: 0,
            direction: 0,
            errors: false,
            obstructions: false,
            target_floor: None
        };
    }
    pub fn send_broadcast(&self) {
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
        socket.set_broadcast(true).expect("Failed to enable broadcast");
        
        let broadcast = Broadcast { id: self.id, role: self.role, status: self.status };
        let message = serde_json::to_string(&broadcast).expect("Failed to serialize broadcast");
        socket.send_to(message.as_bytes(), BROADCAST_ADDR).expect("Failed to send broadcast");
    }

    pub fn receive_broadcasts() -> Option<Broadcast> {
        let socket = UdpSocket::bind(BROADCAST_ADDR).expect("Failed to bind socket");
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