use crate::prelude::*;
use std::net::UdpSocket;
use std::str;

const BROADCAST_ADDR: &str = "255.255.255.255:20010";
const LISTEN_ADDR: &str = "0.0.0.0:20010";
pub const ID: u8 = 10;

pub const MASTER: u8 = 0;
pub const MASTER_BACKUP: u8 = 1;
pub const SLAVE: u8 = 2;

#[derive(Clone, Debug)]
pub struct NetworkUnit {
    pub id: u8,
    pub role: u8,
    pub my_master: Option<u8>,
    pub status_list: HashSet<Status>
}

impl NetworkUnit {
    pub fn new(id:u8) -> Self {
        let (role,my_master) = NetworkUnit::determine_role(id);
        return NetworkUnit{
            id,
            role,
            my_master,
            status_list: HashSet::<Status>::new()
        }
    }
    pub fn determine_role(id:u8) -> (u8,Option<u8>) {
        return (MASTER, None);
    }
    pub fn send_broadcast(&self,broadcast:Communication) {
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind socket");
        socket.set_broadcast(true).expect("Failed to enable broadcast");
        
        let message = serde_json::to_string(&broadcast).expect("Failed to serialize broadcast");
        socket.send_to(message.as_bytes(), BROADCAST_ADDR).expect("Failed to send broadcast");
    }

    pub fn receive_broadcast(&self) -> Option<Communication> {
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

pub fn network_periodic_sender(network_unit: NetworkUnit, network_channel_rx: Receiver<Communication>) {
    let mut message: Option<Communication> = None;

    loop {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0").and_then(|s| { s.set_broadcast(true).map(|_| s) }) {
            let mut restart = false;
            
            while !restart {
                cbc::select! {
                    recv(network_channel_rx) -> msg => {
                        message = msg.ok();
                    },
                    default(Duration::from_millis(100)) => {}
                }

                if let Some(msg) = &message {
                    if serde_json::to_string(msg).is_err() ||
                       socket.send_to(&serde_json::to_string(msg).unwrap().as_bytes(), BROADCAST_ADDR).is_err()
                    {
                        restart = true;
                    }
                }
            }
        }
        
        sleep(Duration::from_secs(1)); // Wait before restarting
    }
}

pub fn network_receiver(network_unit: NetworkUnit, network_channel_tx: Sender<Communication>) {
    loop {
        // Create new socket each iteration to recover from errors
        let socket = match UdpSocket::bind(LISTEN_ADDR) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to bind socket: {}, retrying...", e);
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        // Set timeout for receiving
        if let Err(e) = socket.set_read_timeout(Some(Duration::from_secs(5))) {
            eprintln!("Failed to set timeout: {}, retrying...", e);
            continue;
        }

        let mut buf = [0; 1024];
            
        loop {
            match socket.recv_from(&mut buf) {
                Ok((size, _)) => {
                    // Attempt to parse the message
                    if let Ok(received) = str::from_utf8(&buf[..size]) {
                        if let Ok(msg) = serde_json::from_str::<Communication>(received) {
                            if msg.target == network_unit.id || msg.target == TARGET_ALL {
                                let _ = network_channel_tx.send(msg);
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Timeout occurred, continue waiting
                    continue;
                }
                Err(e) => {
                    eprintln!("Receive error: {}, restarting...", e);
                    break;
                }
            }
        }
    }
}