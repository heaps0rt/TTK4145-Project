use std::default;
use std::thread::*;
use std::time::*;
use std::collections::HashSet;
use std::u8;
use std::sync::*;

use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use crossbeam_channel as cbc;

use driver_rust::elevio;
use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio::elev::DIRN_DOWN;
use driver_rust::elevio::elev::DIRN_STOP;
use driver_rust::elevio::elev::DIRN_UP;
use driver_rust::elevio::elev::HALL_DOWN;
use driver_rust::elevio::elev::HALL_UP;
use driver_rust::elevio::elev as e;
use driver_rust::elevio::poll;
use driver_rust::elevio::poll::CallButton;

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub struct Order {
    pub floor_number: u8,
    pub direction: u8
}

impl Order {
    pub fn new() -> Self {
        Order{
            floor_number: u8::MAX,
            direction: DIRN_STOP
        }
    }
}

fn print_order(order: &Order) -> () {
    let floor = order.floor_number;
    let direction = order.direction;
    println!("Floor: \n{:#?}", floor);
    println!("Direction: \n{:#?}", direction);

}

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub struct Status {
    pub last_floor: u8,
    pub direction: u8,
    pub errors: bool, // Yes or no, any errors
    pub obstructions: bool // Yes or no, any obstructions
}

impl Status {
    pub fn new() -> Self {
        Status{
            last_floor: u8::MAX,
            direction: u8::MAX,
            errors: false,
            obstructions: false
        }
    }
}

// Const variables for use in comms
pub const STATUS_MESSAGE: u8 = 0;
pub const ORDER_TRANSFER: u8 = 1;
pub const ORDER_ACK: u8 = 2;

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub struct Communication {
    pub sender: u8,
    pub target: u8,
    pub comm_type: u8,
    pub status: Option<Status>,
    pub order: Option<Order>
}

// Self explanatory, checks what lights are on and turns off the correct ones
fn check_lights(elevator: &Elevator, dirn: u8, floor: u8, num_floors: u8) -> () {
    elevator.call_button_light(floor, e::CAB, false);
    if dirn == e::DIRN_DOWN || floor == (num_floors-1) {
        elevator.call_button_light(floor, e::HALL_DOWN, false);
    }
    else if dirn == e::DIRN_UP || floor == 0 {
        elevator.call_button_light(floor, e::HALL_UP, false);
    }
    else if dirn == e::DIRN_STOP {
        elevator.call_button_light(floor, e::HALL_DOWN, false);
        elevator.call_button_light(floor, e::HALL_UP, false);
    }
}

// Sends orders to the elevator. Currently fucked
fn order_up(comms_channel_tx: Sender<Communication>, order_list_w_copy: HashSet<Order>, status_list: &RwLockReadGuard<'_, Vec<Status>>) -> () {
    let mut sent_direction: u8 = u8::MAX;
    for element in &order_list_w_copy {

    }
}

// Master function. Runs forever (or till it panics)
fn run_master(elev_num_floors: u8, elevator: Elevator, poll_period: Duration, comms_channel_tx: Sender<Communication>, comms_channel_rx: Receiver<Communication>) -> () {

    let (call_button_tx, call_button_rx) = cbc::unbounded::<elevio::poll::CallButton>(); // Initialize call buttons
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::call_buttons(elevator, call_button_tx, poll_period));
    }

    // Setting up prder set and status list with Rwlock
    // Rwlock means that it can either be written to by a single thread or read by any number of threads at once
    let mut order_list = RwLock::from(HashSet::new());
    let mut status_list = RwLock::from(Vec::from([Status::new(), Status::new(), Status::new()]));
    let mut elevator_direction = e::DIRN_STOP;

    // Main master loop
    loop {
        // Crossbeam channel runs the main functions of the master
        // It constantly checks whether it has received a message and runs a standard function if it has waited too long
        cbc::select! {
            // Get info from call button and add it to the list of floors ordered if it is a hall call
            recv(call_button_rx) -> a => { 
                let call_button = a.unwrap();
                // If call is a hall call, add it
                if call_button.call == e::HALL_DOWN || call_button.call == e::HALL_UP {
                    let new_order = Order {
                        floor_number: call_button.floor,
                        direction: call_button.call
                    };

                    
                    let mut order_list_w = order_list.write().unwrap();
                    order_list_w.insert(new_order);
                    elevator.call_button_light(call_button.floor, call_button.call, true);

                }
            }

            // Get info from comms_channel and process according to status if it is meant for us
            recv(comms_channel_rx) -> a => {
                let message = a.unwrap();
                // println!("Received message: {:#?}", message.comm_type);
                if message.target == u8::MAX {
                    match message.comm_type {
                        STATUS_MESSAGE => {
                            // println!("Received status: {:#?}", message.status);
                            let mut status_list_w = status_list.write().unwrap();
                            status_list_w[message.sender as usize] = message.status.unwrap();
                        }
                        ORDER_TRANSFER => {
                            // Message is not for me
                        }
                        ORDER_ACK => {
                            let mut order_list_w = order_list.write().unwrap();
                            if order_list_w.contains(&message.order.unwrap()) {
                                order_list_w.remove(&message.order.unwrap());
                            }
                            else {
                                println!("Feil i ack av order")
                            }
                        }
                        3_u8..=u8::MAX => {
                            println!("Feil i meldingssending")
                        }
                    }
                }
            }
            // This function polls continuously if no other functions have been called
            default(Duration::from_millis(500)) => {
                // Opening status list for reading
                let mut status_list_r = status_list.read().unwrap();

                // If status has been received, ie. elevator is alive, try to send orders
                if status_list_r[0 as usize].direction != u8::MAX {
                    // Opening order list for reading and cloning it
                    // This should be done differently, order_list is hogged
                    let mut order_list_r = order_list.read().unwrap();
                    let mut order_list_r_copy = order_list_r.clone();
                    let comms_channel_tx = comms_channel_tx.clone();
                    // Calling ordering function
                    order_up(comms_channel_tx, order_list_r_copy, &status_list_r);
                }
                println!("{:#?}", status_list);

                
            }
        }
    }
}

// Elevator function. Runs forever (or till it panics)
fn run_elevator(elev_num_floors: u8, elevator: Elevator, poll_period: Duration, comms_channel_tx: Sender<Communication>, comms_channel_rx: Receiver<Communication>) -> () {

    // Initialize call buttons
    let (call_button_tx, call_button_rx) = cbc::unbounded::<elevio::poll::CallButton>();
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::call_buttons(elevator, call_button_tx, poll_period));
    }
    // Initialize floor sensor
    let (floor_sensor_tx, floor_sensor_rx) = cbc::unbounded::<u8>(); 
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::floor_sensor(elevator, floor_sensor_tx, poll_period));
    }
    // Initialize stop button
    let (stop_button_tx, stop_button_rx) = cbc::unbounded::<bool>(); 
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::stop_button(elevator, stop_button_tx, poll_period));
    }
    // Initialize obstruction switch
    let (obstruction_tx, obstruction_rx) = cbc::unbounded::<bool>(); 
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::obstruction(elevator, obstruction_tx, poll_period));
    }

    // Set up direction variable
    let mut dirn = e::DIRN_DOWN; 
    // Send the elevator down upon startup
    elevator.motor_direction(dirn);
    println!("På vei ned");
    
    // Set up variable to remember what floor we were last at
    let mut last_floor: u8 = elev_num_floors+1;

    // Setting up destination set with Rwlock
    // Rwlock means that it can either be written to by a single thread or read by any number of threads at once
    let mut destination_list:RwLock<HashSet<Order>> = RwLock::from(HashSet::new());

    // The main running loop of the elevator
    loop {
        // Crossbeam channel runs the main functions of the master
        // It constantly checks whether it has received a message and runs a standard function if it has waited too long
        cbc::select! {
            // Get info from call button and add it to the destination list if it is a cab call
            recv(call_button_rx) -> a => { 
                let call_button = a.unwrap();
                if call_button.call == e::CAB {
                    if call_button.floor < last_floor {
                        let new_order = Order {
                            floor_number: call_button.floor,
                            direction: e::HALL_DOWN
                        };
                        let mut destination_list_w = destination_list.write().unwrap();
                        destination_list_w.insert(new_order);
                        elevator.call_button_light(call_button.floor, call_button.call, true);
                        sleep(Duration::from_millis(100));
                    }
                    else if call_button.floor >= last_floor {
                        let new_order = Order {
                            floor_number: call_button.floor,
                            direction: e::HALL_UP
                        };
                        let mut destination_list_w = destination_list.write().unwrap();
                        destination_list_w.insert(new_order);
                        elevator.call_button_light(call_button.floor, call_button.call, true);
                        sleep(Duration::from_millis(100));
                    }  
                }
            }
            // Get floor status and save last floor for later use
            recv(floor_sensor_rx) -> a => { 
                let floor = a.unwrap();
                //println!("Floor: {:#?}", floor);
                last_floor = floor;
                //println!("Last floor updated to: {:#?}", last_floor);

                let mut destination_list_w = destination_list.write().unwrap();

                let order_check = Order {
                    floor_number: floor,
                    direction: dirn
                };
                if destination_list_w.contains(&order_check){
                    elevator.motor_direction(e::DIRN_STOP);
                    println!("Stopper midlertidig");
                    destination_list_w.remove(&order_check);

                    elevator.door_light(true);
                    sleep(Duration::from_millis(500));
                    elevator.door_light(false);

                    if destination_list_w.is_empty(){
                        dirn = e::DIRN_STOP;
                    }
                    elevator.motor_direction(dirn);
                    println!("Fortsetter");
                    
                }
                if dirn == e::DIRN_UP && floor == (elev_num_floors-1) {
                    dirn = e::DIRN_STOP;
                    elevator.motor_direction(dirn);
                }
                else if dirn == e::DIRN_DOWN && floor == 0 {
                    dirn = e::DIRN_STOP;
                    elevator.motor_direction(dirn);
                }
                check_lights(&elevator, dirn, floor, elev_num_floors);
            }
            // Get info from comms_channel and process according to status if it is meant for us
            recv(comms_channel_rx) -> a => {
                let message = a.unwrap();
                if message.target == 0 {
                    match message.comm_type {
                        STATUS_MESSAGE => {
                            // Message is not for me
                        }
                        ORDER_TRANSFER => {
                            let new_order = message.order.unwrap();

                            let mut destination_list_w = destination_list.write().unwrap();
                            destination_list_w.insert(new_order);
                            let mut new_message = message;
                            new_message.target = new_message.sender;
                            new_message.sender = 0;
                            new_message.comm_type = ORDER_ACK;
                            comms_channel_tx.send(new_message).unwrap();
                            sleep(Duration::from_millis(100));
                        }
                        ORDER_ACK => {
                            // Message is not for me
                        }
                        3_u8..=u8::MAX => {
                            println!("Feil i meldingssending")
                        }
                    }
                }
            }
            // This function polls continuously
            default(Duration::from_millis(200)) => {

                if dirn == e::DIRN_STOP {
                    let destination_list_r = destination_list.read().unwrap();
                    let destination_list_r_copy = destination_list_r.clone();
                    if !destination_list_r.is_empty() {
                        let _top_floor = elev_num_floors-1;
                        if last_floor == 0 {
                            for destination in destination_list_r_copy {
                                if destination.direction != e::HALL_DOWN {
                                    dirn = e::DIRN_UP;
                                    elevator.motor_direction(dirn);
                                    break;
                                }
                            }
                        }
                        else if last_floor == (elev_num_floors-1) {
                            for destination in destination_list_r_copy {
                                if destination.direction != e::HALL_UP {
                                    dirn = e::DIRN_DOWN;
                                    elevator.motor_direction(dirn);
                                    break;
                                }
                            }
                        }
                        else {
                            for destination in destination_list_r_copy {
                                if destination.direction == e::HALL_UP {
                                    dirn = e::DIRN_UP;
                                    elevator.motor_direction(dirn);
                                    break;
                                }
                                if destination.direction == e::HALL_DOWN {
                                    dirn = e::DIRN_DOWN;
                                    elevator.motor_direction(dirn);
                                    break;
                                }
                            }
                        }
                    }

                }
                

                // Create and send status to master
                let current_status = Status {
                    last_floor: last_floor,
                    direction: dirn,
                    errors: false,
                    obstructions: false
                };
                let new_message = Communication {
                    sender: 0,
                    target: u8::MAX,
                    comm_type: STATUS_MESSAGE,
                    status: Some(current_status),
                    order: Some(Order::new())
                };
                comms_channel_tx.send(new_message).unwrap();

                // New scope to hog destination list as little as possible
                {
                // Status update readout, mostly for debugging
                let destination_list_r = destination_list.read().unwrap();
                println!("\n\n\n\n\n");
                println!("-------Status---------");
                println!("Destinasjoner: {:#?}", destination_list_r);
                match dirn{
                    e::DIRN_DOWN => {
                        println!("Retning: Ned");
                    }
                    e::DIRN_UP => {
                        println!("Retning: Opp");
                    }
                    e::DIRN_STOP => {
                        println!("Retning: Stoppet");
                    }
                    2_u8..=254_u8 => {
                        println!("Noe har gått kraftig galt");
                    }
                }
                println!("-------Slutt----------");
            }
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    // Setting up durations for later use
    let five_hundred_millis = Duration::from_millis(500);
    let now = Instant::now();
    
    // Set floor count
    let elev_num_floors = 4; 

    // Initialize and connect elevator
    let elevator = e::Elevator::init("localhost:15657", elev_num_floors)?; 
    println!("Elevator started:\n{:#?}", elevator);

    // Set up communication channel, this is just a substitute for network communication we will implement later
    let (comms_channel_tx, comms_channel_rx) = cbc::unbounded(); 

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
        run_master(elev_num_floors, elevator, poll_period, comms_channel_tx, comms_channel_rx);
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
        run_elevator(elev_num_floors, elevator, poll_period, comms_channel_tx, comms_channel_rx);
    });
    }

    // Main thread just loops so program doesn't shut down
    // Error handling goes here eventually
    loop {
        sleep(five_hundred_millis);
        assert!(now.elapsed() >= five_hundred_millis);
   }
}
