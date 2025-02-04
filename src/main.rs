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

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
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

// Const variables for use in comms
pub const STATUS_MESSAGE: u8 = 0;
pub const ORDER_TRANSFER: u8 = 1;
pub const ORDER_ACK: u8 = 2;

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct Communication {
    pub sender: u8,
    pub target: u8,
    pub comm_type: u8,
    pub status: Option<Status>,
    pub order: Option<Order>
}

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

fn order_up(comms_channel_tx: Sender<Communication>, order_list_w_copy: HashSet<Order>, status_list: &RwLockReadGuard<'_, Vec<Status>>) -> () {
    let mut sent_direction: u8 = u8::MAX;
    for element in &order_list_w_copy {
        match status_list[0 as usize].direction {
            DIRN_STOP => {
                if sent_direction == u8::MAX && element.floor_number != status_list[0 as usize].last_floor {
                    let new_message = Communication {
                        sender: u8::MAX,
                        target: 0,
                        comm_type: ORDER_TRANSFER,
                        order: Some(*element),
                        status: Some(status_list[0 as usize])
                    };
                    comms_channel_tx.send(new_message).unwrap();
                    sent_direction = element.direction;
                }
                else if sent_direction == element.direction {
                    if sent_direction == DIRN_DOWN && element.floor_number < status_list[0 as usize].last_floor {
                        let new_message = Communication {
                            sender: u8::MAX,
                            target: 0,
                            comm_type: ORDER_TRANSFER,
                            order: Some(*element),
                            status: Some(status_list[0 as usize])
                        };
                        comms_channel_tx.send(new_message).unwrap();
                    }
                    else if sent_direction == DIRN_UP && element.floor_number > status_list[0 as usize].last_floor {
                        let new_message = Communication {
                            sender: u8::MAX,
                            target: 0,
                            comm_type: ORDER_TRANSFER,
                            order: Some(*element),
                            status: Some(status_list[0 as usize])
                        };
                        comms_channel_tx.send(new_message).unwrap();
                    }
                }
                
            }
            DIRN_UP => {
                if element.floor_number > status_list[0 as usize].last_floor {
                    let new_message = Communication {
                        sender: u8::MAX,
                        target: 0,
                        comm_type: ORDER_TRANSFER,
                        order: Some(*element),
                        status: Some(status_list[0 as usize])
                    };
                    comms_channel_tx.send(new_message).unwrap();
                }
            }
            DIRN_DOWN => {
                if element.floor_number < status_list[0 as usize].last_floor {
                    let new_message = Communication {
                        sender: u8::MAX,
                        target: 0,
                        comm_type: ORDER_TRANSFER,
                        order: Some(*element),
                        status: Some(status_list[0 as usize])
                    };
                    comms_channel_tx.send(new_message).unwrap();
                }
            }
            2_u8..=254_u8 => {
                println!("Error in status")
            }
        }
    }
}

fn run_master(elev_num_floors: u8, elevator: Elevator, poll_period: Duration, comms_channel_tx: Sender<Communication>, comms_channel_rx: Receiver<Communication>) -> () {

    // Setting up durations for later use
    let a_hundred_millis = Duration::from_millis(100);

    let (call_button_tx, call_button_rx) = cbc::unbounded::<elevio::poll::CallButton>(); // Initialize call buttons
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::call_buttons(elevator, call_button_tx, poll_period));
    }

    // Setting up set with Rwlock
    // Rwlock means that it can either be written to by a single thread or read by any number of threads at once
    let mut order_list = RwLock::from(HashSet::new());
    let mut status_list = RwLock::from(Vec::new());
    let mut elevator_direction = e::DIRN_STOP;

    loop {
        cbc::select! {
            recv(call_button_rx) -> a => { // Get info from call button and add it to the list of floors ordered if it is a hall call
                let call_button = a.unwrap();
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
            recv(comms_channel_rx) -> a => {
                let message = a.unwrap();
                if message.target == u8::MAX {
                    match message.comm_type {
                        STATUS_MESSAGE => {
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
            // This function polls continuously
            default(a_hundred_millis) => {
                //Ordering function
                let mut status_list_r = status_list.read().unwrap();
                if !status_list_r.is_empty(){
                    let mut order_list_w = order_list.write().unwrap();
                    let mut order_list_w_copy = order_list_w.clone();
                    let comms_channel_tx = comms_channel_tx.clone();
                    order_up(comms_channel_tx, order_list_w_copy, &status_list_r);
                }

                
            }
        }
    }
}

fn run_elevator(elev_num_floors: u8, elevator: Elevator, poll_period: Duration, comms_channel_tx: Sender<Communication>, comms_channel_rx: Receiver<Communication>) -> () {

    // Setting up durations for later use
    let a_hundred_millis = Duration::from_millis(100);
    let five_hundred_millis = Duration::from_millis(500);
    let now = Instant::now();

    let (call_button_tx, call_button_rx) = cbc::unbounded::<elevio::poll::CallButton>(); // Initialize call buttons
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::call_buttons(elevator, call_button_tx, poll_period));
    }
    let (floor_sensor_tx, floor_sensor_rx) = cbc::unbounded::<u8>(); // Initialize floor sensor
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::floor_sensor(elevator, floor_sensor_tx, poll_period));
    }
    let (stop_button_tx, stop_button_rx) = cbc::unbounded::<bool>(); // Initialize stop button
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::stop_button(elevator, stop_button_tx, poll_period));
    }
    let (obstruction_tx, obstruction_rx) = cbc::unbounded::<bool>(); // Initialize obstruction switch
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::obstruction(elevator, obstruction_tx, poll_period));
    }

    let mut dirn = e::DIRN_DOWN; // Set mutable direction
    // If the elevator is not on a floor, send it down
    elevator.motor_direction(dirn);
    println!("På vei ned");
    

    let mut last_floor: u8 = elev_num_floors+1;
    // Setting up set with Rwlock
    // Rwlock means that it can either be written to by a single thread or read by any number of threads at once
    let mut destination_list = RwLock::from(HashSet::from([0]));
    loop {
        cbc::select! {
            recv(call_button_rx) -> a => { // Get info from call button and add it to the destination list if it is a cab call
                let call_button = a.unwrap();
                if call_button.call == e::CAB {
                    match dirn {
                        e::DIRN_DOWN => {
                            if call_button.floor < last_floor {
                                let mut destination_list_w = destination_list.write().unwrap();
                                destination_list_w.insert(call_button.floor);
                                elevator.call_button_light(call_button.floor, call_button.call, true);
                            }
                        }
                        e::DIRN_UP => {
                            if call_button.floor > last_floor {
                                let mut destination_list_w = destination_list.write().unwrap();
                                destination_list_w.insert(call_button.floor);
                                elevator.call_button_light(call_button.floor, call_button.call, true);
                            }
                        }
                        e::DIRN_STOP => {
                            if call_button.floor != last_floor {
                                let mut destination_list_w = destination_list.write().unwrap();
                                destination_list_w.insert(call_button.floor);
                                elevator.call_button_light(call_button.floor, call_button.call, true);
                            }
                        }
                        2_u8..=254_u8 => {
                            println!("Error in cab order")
                        }
                    }
                    
                }
            }
            recv(floor_sensor_rx) -> a => { // Get floor status and save last floor for later use
                let floor = a.unwrap();
                //println!("Floor: {:#?}", floor);
                last_floor = floor;
                //println!("Last floor updated to: {:#?}", last_floor);
                check_lights(&elevator, dirn, floor, elev_num_floors);

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

                let mut destination_list_w = destination_list.write().unwrap();
                
                if destination_list_w.contains(&floor){
                    elevator.motor_direction(e::DIRN_STOP);
                    println!("Stopper midlertidig");
                    destination_list_w.remove(&floor);

                    elevator.door_light(true);
                    sleep(five_hundred_millis);
                    assert!(now.elapsed() >= five_hundred_millis);
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
            }
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
                            destination_list_w.insert(new_order.floor_number);
                            let mut new_message = message;
                            new_message.comm_type = ORDER_ACK;
                            comms_channel_tx.send(new_message).unwrap();
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
            default(a_hundred_millis) => {
                let mut destination_list_r = destination_list.read().unwrap();
                let mut destination_list_r_copy = destination_list_r.clone();
                let mut destination_list_vec = Vec::from_iter(destination_list_r_copy);
                let min = destination_list_vec.iter().min();
                let max = destination_list_vec.iter().max();

                if dirn == DIRN_STOP && !destination_list_r.is_empty() {
                    if *max.unwrap() < last_floor {
                        dirn = DIRN_DOWN;
                        elevator.motor_direction(dirn);
                    }
                    else if *min.unwrap() > last_floor {
                        dirn = DIRN_UP;
                        elevator.motor_direction(dirn);
                    }
                }

                // Status update readout
                let destination_list_r = destination_list.read().unwrap();
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

fn main() -> std::io::Result<()> {
    // Setting up durations for later use
    let a_hundred_millis = Duration::from_millis(100);
    let five_hundred_millis = Duration::from_millis(500);
    let now = Instant::now();
    
    let elev_num_floors = 4; // Set floor count
    let elevator = e::Elevator::init("localhost:15657", elev_num_floors)?; // Initialize and connect elevator
    println!("Elevator started:\n{:#?}", elevator);

    let (comms_channel_tx, comms_channel_rx) = cbc::unbounded(); // communication channel

    let poll_period = Duration::from_millis(25); // Set poll period for buttons and sensors
    {
    let elevator = elevator.clone();
    let comms_channel_tx = comms_channel_tx.clone();
    let comms_channel_rx = comms_channel_rx.clone();
    spawn(move || {
        run_master(elev_num_floors, elevator, poll_period, comms_channel_tx, comms_channel_rx);
    });
    }

    {
    // Cloning critical variables
    // Note that for all of these, cloning only creates a seperate handle, not a new variable
    let elevator = elevator.clone();
    let comms_channel_tx = comms_channel_tx.clone();
    let comms_channel_rx = comms_channel_rx.clone();
    spawn(move || {
        run_elevator(elev_num_floors, elevator, poll_period, comms_channel_tx, comms_channel_rx);
    });
    }
    
   loop {
    sleep(five_hundred_millis);
    assert!(now.elapsed() >= five_hundred_millis);
   }
}
