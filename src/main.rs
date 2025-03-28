use ttk4145_project::prelude::*;
use ttk4145_project::network::server::*;

fn network_periodic_sender(network_unit:NetworkUnit,network_channel_rx: Receiver<Communication>) {
    let mut message:Option<Communication> = None;
    loop {
        cbc::select! {
            recv(network_channel_rx) -> a => {
                message = a.ok();
                if let Some(msg) = message {
                    network_unit.send_broadcast(msg.clone());
                }
            }
            default(Duration::from_millis(100)) => {
                if let Some(msg) = message {
                    network_unit.send_broadcast(msg.clone());
                }
            }
        }
    }
}

fn network_reciever(network_unit:NetworkUnit,internal_order_channel_tx:Sender<InternalCommunication>, mut status_list_w: RwLockWriteGuard<'_, Vec<Status>>) -> () {
    loop {
        let message: Option<Communication> = network_unit.receive_broadcast();
        if let Some(msg) = message {
            if (msg.target == network_unit.id) || (msg.target == TARGET_ALL) {
                match msg.comm_type {
                    STATUS_MESSAGE => { // writes status message to the status_list
                        // println!("Received status: {:#?}", message.status);
                        status_list_w[msg.sender as usize] = msg.status.unwrap();
                    }
                    ORDER_TRANSFER => {
                        // Message is not for me
                    }
                    ORDER_ACK => { // Sends message to order memory in order to delete acknowledged order.
                        let new_comm = InternalCommunication {
                            intention: DELETE,
                            order: msg.order
                        };
                        internal_order_channel_tx.send(new_comm).unwrap();
                    }
                    3_u8..=u8::MAX => {
                        println!("Feil i meldingssending")
                    }
                }
            }
        }
    }
}

fn main() -> std::io::Result<()>{
    // Setting up durations for later use
    let five_hundred_millis = Duration::from_millis(500);
    let now = Instant::now();
    
    // Set floor count
    let elev_num_floors = 4;

    // Initialize and connect elevator
    let elevator = e::Elevator::init("localhost:15657", elev_num_floors)?;
    println!("Elevator started:\n{:#?}", elevator);

    // Set up communication channel, this is just a substitute for network communication we will implement later
    let (network_channel_tx, network_channel_rx) = cbc::unbounded::<Communication>();
    let (master_channel_tx, master_channel_rx) = cbc::unbounded::<Communication>();

    // Initialize network unit
    let network_unit = NetworkUnit::new(ID);

    {
    let network_unit:NetworkUnit = network_unit.clone();
    let network_channel_rx: Receiver<Communication> = network_channel_rx.clone();
    spawn(move || {network_periodic_sender(network_unit,network_channel_rx);});
    }

    // Set poll period for buttons and sensors
    let poll_period = Duration::from_millis(25);

    // New scope so cloned values only stay inside it
    {
    // Cloning critical variables
    // Note that for all of these, cloning only creates a seperate handle, not a new variable
    let network_channel_tx = network_channel_tx.clone();
    let master_channel_rx = master_channel_rx.clone();
    // Starting a thread which runs the master and starts the necessary threads
    spawn(move || {
        ttk4145_project::client::master::run_master(network_channel_tx, master_channel_rx);
    });
    }

    // New scope so cloned values only stay inside it
    {
    // Cloning critical variables
    // Note that for all of these, cloning only creates a seperate handle, not a new variable
    let elevator = elevator.clone();
    let comms_channel_tx = network_channel_tx.clone();
    let comms_channel_rx = network_channel_rx.clone();
    
    // Starting a thread which runs the elevator and starts the necessary threads
    spawn(move || {
        ttk4145_project::client::elevator::run_elevator(network_unit.id,elev_num_floors, elevator, poll_period, comms_channel_tx, comms_channel_rx);
    });
    }

    // Main thread just loops so program doesn't shut down
    // Error handling goes here eventually
    loop {
        sleep(five_hundred_millis);
        assert!(now.elapsed() >= five_hundred_millis);
   }
}