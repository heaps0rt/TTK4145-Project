use ttk4145_project::prelude::*;
use ttk4145_project::network::server::*;

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
    let (network_send_channel_tx, network_send_channel_rx) = cbc::unbounded::<Communication>();
    let (master_channel_tx, master_channel_rx) = cbc::unbounded::<Communication>();
    let (elevator_channel_tx, elevator_channel_rx) = cbc::unbounded::<Communication>();

    // Initialize network unit
    let mut network_unit = NetworkUnit::new(ID);

    // Initialize network reciever
    {
    let network_unit:NetworkUnit = network_unit.clone();
    let master_channel_tx: Sender<Communication> = master_channel_tx.clone();
    let elevator_channel_tx: Sender<Communication> = elevator_channel_tx.clone();
    spawn(move || {network_receiver(network_unit, master_channel_tx,elevator_channel_tx);});
    }

    // Listen for statuses a bit before determining starting role
    {
        sleep(Duration::from_millis(3000));
        network_unit.update_role();
        println!("Set role as {}:",network_unit.role)
    }

    // Initialize network sender
    {
        let network_unit:NetworkUnit = network_unit.clone();
        let network_send_channel_rx: Receiver<Communication> = network_send_channel_rx.clone();
        spawn(move || {network_periodic_sender(network_unit,network_send_channel_rx);});
        }

    // Set poll period for buttons and sensors
    let poll_period = Duration::from_millis(25);

    // New scope so cloned values only stay inside it
    if network_unit.role == MASTER {
    // Cloning critical variables
    // Note that for all of these, cloning only creates a seperate handle, not a new variable
    let network_unit=network_unit.clone();
    let network_channel_tx = network_send_channel_tx.clone();
    let master_channel_rx = master_channel_rx.clone();
    // Starting a thread which runs the master and starts the necessary threads
    spawn(move || {
        ttk4145_project::client::master::run_master(network_unit,network_channel_tx, master_channel_rx);
    });
    println!("Master initiated.")
    }

    // New scope so cloned values only stay inside it
    {
    // Cloning critical variables
    // Note that for all of these, cloning only creates a seperate handle, not a new variable
    let elevator = elevator.clone();
    let network_unit=network_unit.clone();
    let network_channel_tx = network_send_channel_tx.clone();
    let elevator_channel_rx = elevator_channel_rx.clone();
    
    // Starting a thread which runs the elevator and starts the necessary threads
    spawn(move || {
        ttk4145_project::client::elevator::run_elevator(network_unit.id,elev_num_floors, elevator, poll_period, network_channel_tx, elevator_channel_rx);
    });
    }

    // Main thread just loops so program doesn't shut down
    // Error handling goes here eventually
    loop {
        sleep(five_hundred_millis);
        assert!(now.elapsed() >= five_hundred_millis);
   }
}