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
            role: MASTER,
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
    pub fn update_role(&mut self) {
        let state_list = self.state_list.lock().unwrap();
        let has_master = state_list.iter().any(|s| s.role == MASTER);
        let has_master_backup = state_list.iter().any(|s| s.role == MASTER_BACKUP);
    
        let (new_role, master_id) = if !has_master {
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
        };
        
        self.role = new_role;
        self.my_master = master_id;
    }
}

pub fn network_periodic_sender(network_unit: NetworkUnit, network_channel_rx: Receiver<Communication>) {
    loop {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0")
            .and_then(|s| s.set_broadcast(true).map(|_| s)) 
        {
            let mut restart = false;
            let mut current_message: Option<Communication> = None; // <-- Mutable holder
            
            while !restart {
                // Check for new messages first
                if let Ok(msg) = network_channel_rx.try_recv() {
                    let mut msg = msg;
                    msg.sender = network_unit.id;
                    msg.sender_role = network_unit.role;
                    // println!("Sending message: {:#?}", msg);
                    current_message = Some(msg);
                }

                // Send the message if we have one
                if let Some(msg) = &current_message {
                    let json = match serde_json::to_string(msg) {
                        Ok(j) => j,
                        Err(e) => {
                            eprintln!("JSON error: {}", e);
                            restart = true;
                            continue;
                        }
                    };
                    
                    if let Err(e) = socket.send_to(json.as_bytes(), BROADCAST_ADDR) {
                        eprintln!("Send error: {}", e);
                        restart = true;
                    }
                    
                    current_message = None;
                }

                sleep(Duration::from_millis(100));
            }
        }
        sleep(Duration::from_secs(1));
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
            // println!("RECIEVING:::");
            match socket.recv_from(&mut buf) {
                Ok((size, _)) => {
                    // Attempt to parse the message
                    if let Ok(received) = str::from_utf8(&buf[..size]) {
                        // println!("received");
                        if let Ok(msg) = serde_json::from_str::<Communication>(received) {
                            // println!("recieved: {:#?}",msg);
                            let network_unit = network_unit.clone();
                            let master_channel_tx = master_channel_tx.clone();
                            let elevator_channel_tx = elevator_channel_tx.clone();
                            network_message_handler(network_unit,msg,master_channel_tx,elevator_channel_tx)
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
    // println!("Recieved message {:#?}", message);
    match message.target {
        MASTER => {
            if network_unit.role == MASTER {
                println!("Sent to master {:#?}", message);
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