use crate::prelude::*;
use std::net::UdpSocket;
use std::str;

const BROADCAST_ADDR: &str = "255.255.255.255:20010";
const LISTEN_ADDR: &str = "0.0.0.0:20010";
pub const ID: u8 = 10;

#[derive(Clone, Debug)]
pub struct NetworkUnit {
    pub id: u8,
    pub role: u8,
    pub my_master: Option<u8>,
    pub state_list: Arc<Mutex<HashSet<State>>>
}

impl NetworkUnit {
    pub fn new(id:u8) -> Self {
        NetworkUnit {
            id,
            role: SLAVE,
            my_master: None,
            state_list: Arc::new(Mutex::new(HashSet::new())),
        }
    }
    pub fn update_state_list(&self, new_state: State) {
        let mut state_list = self.state_list.lock().unwrap();
        // Remove existing state with the same id
        if let Some(existing) = state_list.iter().find(|s| s.id == new_state.id).cloned() {
            state_list.remove(&existing);
        }
        state_list.insert(new_state);
    }
    pub fn get_state_list(&self) -> HashSet<State> {
        self.state_list.lock().unwrap().clone()
    }
    pub fn determine_role(&self) -> (u8, Option<u8>) {
        let state_list = self.state_list.lock().unwrap();
        let has_master = state_list.iter().any(|s| s.role == MASTER);
        let has_master_backup = state_list.iter().any(|s| s.role == MASTER_BACKUP);
    
        if !has_master {
            // No master found, become master
            (MASTER, Some(self.id))
        } else if has_master && !has_master_backup {
            // Master exists but no backup, become master_backup
            let master_id = state_list.iter()
                .find(|s| s.role == MASTER)
                .map(|s| s.id);
            (MASTER_BACKUP, master_id)
        } else {
            // Both master and backup exist, become slave
            let master_id = state_list.iter()
                .find(|s| s.role == MASTER)
                .map(|s| s.id);
            (SLAVE, master_id)
        }
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
    let message: Option<Communication> = None;

    loop {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0").and_then(|s| { s.set_broadcast(true).map(|_| s) }) {
            let mut restart = false;
            
            while !restart {
                cbc::select! {
                    recv(network_channel_rx) -> msg => {
                        if let Some(mut message) = msg.ok() {
                            message.sender = network_unit.id;
                            message.sender_role = network_unit.role;
                        }
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

pub fn network_receiver(network_unit: NetworkUnit, master_channel_tx:Sender<Communication>,elevator_channel_tx:Sender<Communication>) {
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
                                let network_unit = network_unit.clone();
                                let master_channel_tx = master_channel_tx.clone();
                                let elevator_channel_tx = elevator_channel_tx.clone();
                                network_message_handler(network_unit,msg,master_channel_tx,elevator_channel_tx)
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Timeout occurred, continue waiting
                    continue;
                }
                Err(e) => {
                    // eprintln!("Receive error: {}, restarting...", e);
                    break;
                }
            }
        }
    }
}

// Recieves external network communcations and processes based on the comm_type
fn network_message_handler(network_unit: NetworkUnit,message:Communication,master_channel_tx:Sender<Communication>,elevator_channel_tx:Sender<Communication>) {
    match message.target {
        MASTER => {
            if network_unit.role == MASTER {
                let _ = master_channel_tx.send(message);
            }
        }
        ID | TARGET_ALL => {
            match message.comm_type {
                STATUS_MESSAGE => {
                    if let Some(status) = message.status {
                        let new_state = State {
                            id: message.sender,
                            role: message.sender_role,
                            status,
                        };
                        network_unit.update_state_list(new_state);
                    }
                }
                ORDER_TRANSFER => {
                    let _ = elevator_channel_tx.send(message);
                }
                _ => {}
            }
        }
        _ => {}
    }
}