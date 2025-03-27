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

    // Initialize network unit
    let network_unit = NetworkUnit::new(1);

    // Set up communication channel, this is just a substitute for network communication we will implement later
    let (comms_channel_tx, comms_channel_rx) = cbc::unbounded::<Communication>();

    let (network_channel_tx, network_channel_rx) = cbc::unbounded::<Communication>(); 

    // Set poll period for buttons and sensors
    let poll_period = Duration::from_millis(25);

    // New scope so cloned values only stay inside it
    {
    // Cloning critical variables
    // Note that for all of these, cloning only creates a seperate handle, not a new variable
    let elevator = elevator.clone();
    let comms_channel_tx = comms_channel_tx.clone();
    let comms_channel_rx = comms_channel_rx.clone();
    // Starting a thread which runs the master and starts the necessary threads
    spawn(move || {
        ttk4145_project::client::master::run_master(comms_channel_tx, comms_channel_rx);
    });
    } 

    // New scope so cloned values only stay inside it
    {
    // Cloning critical variables
    // Note that for all of these, cloning only creates a seperate handle, not a new variable
    let elevator = elevator.clone();
    let comms_channel_tx = comms_channel_tx.clone();
    let comms_channel_rx = comms_channel_rx.clone();
    
    // Starting a thread which runs the elevator and starts the necessary threads
    spawn(move || {
        ttk4145_project::client::elevator::run_elevator(elev_num_floors, elevator, poll_period, comms_channel_tx, comms_channel_rx);
    });
    }

    // Main thread just loops so program doesn't shut down
    // Error handling goes here eventually
    loop {
        sleep(five_hundred_millis);
        assert!(now.elapsed() >= five_hundred_millis);
   }
}