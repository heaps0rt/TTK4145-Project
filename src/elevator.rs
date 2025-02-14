use std::default;
use std::hash::Hash;
use std::thread::*;
use std::time::*;
use std::collections::HashSet;
use std::u8;
use std::sync::*;
use std::cmp::max;

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
use driver_rust::elevio::poll::floor_sensor;
use driver_rust::elevio::poll::CallButton;

// Libraries we have added go below
use cli_table::{format::Justify, print_stdout, Cell, Style, Table};
use clearscreen;

pub const DIRN_STOP_TEMP: u8 = 3;

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, PartialOrd)]
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

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, PartialOrd)]
pub struct Status {
    pub last_floor: u8,
    pub direction: u8,
    pub errors: bool, // Yes or no, any errors
    pub obstructions: bool, // Yes or no, any obstructions
    pub target_floor: Option<u8>
}

impl Status {
    pub fn new() -> Self {
        Status{
            last_floor: u8::MAX,
            direction: u8::MAX,
            errors: false,
            obstructions: false,
            target_floor: Some(u8::MAX)
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

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub struct InternalCommunication {
    pub intention: u8, // use code words defined below
    pub order: Option<Order>
}

// Const variables for use in internal comms
pub const DELETE: u8 = 0;
pub const INSERT: u8 = 1;
pub const REQUEST_DESTINATION: u8 = 6;
pub const REQUEST_DIRECTION: u8 = 7;
pub const UPDATE_DIRECTION: u8 = 8;

fn direction_to_string(dirn: u8) -> String {
    match dirn {
        e::DIRN_UP => {
            return String::from("Oppover");
        }
        e::DIRN_DOWN => {
            return String::from("Nedover");
        }
        e::DIRN_STOP => {
            return String::from("Stoppet");
        }
        2_u8..=254_u8 => {
            return String::from("Ukjent");
        }
    }
}

fn elevdirn_to_halldirn(dirn: u8) -> u8 {
    let mut direction = 0;
    match dirn {
        e::DIRN_DOWN => {
            direction = e::HALL_DOWN;
        }
        e::DIRN_UP => {
            direction = e::HALL_UP;
        }
        0|2_u8..=254_u8 => {
            println!("Can't convert direction");
        }
    }
    return direction
}

fn halldirn_to_elevdirn(dirn: u8) -> u8 {
    let mut direction = 0;
    match dirn {
        e::HALL_DOWN => {
            direction = e::DIRN_DOWN;
        }
        e::HALL_UP => {
            direction = e::DIRN_UP;
        }
        2_u8..=u8::MAX => {
            println!("Can't convert direction");
        }
    }
    return direction
}

fn opposite_direction_hall(direction: u8) -> u8 {
    let mut new_direction = 0;
    match direction {
        e::HALL_UP => {
            new_direction = e::HALL_DOWN;
        }
        e::HALL_DOWN => {
            new_direction = e::HALL_UP;
        }
        2_u8..=u8::MAX => {
            println!("Can't invert direction");
        }
    }
    return new_direction
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

fn target_floor_function(dirn: u8, destination_list: &HashSet<Order>, last_floor: u8) -> Option<u8> {
    let mut destination_list_vec = Vec::new();
    let mut destination_list_copy = destination_list.clone();
    for order in destination_list_copy {
        destination_list_vec.insert(0, order.floor_number);
    }
    match dirn {
        e::DIRN_UP => {
            let target_floor = destination_list_vec.iter().max();
            return target_floor.copied();
        }
        e::DIRN_DOWN => {
            let target_floor = destination_list_vec.iter().min();
            return target_floor.copied();
        }
        e::DIRN_STOP => {
            return Some(last_floor);
        }
        2_u8..=254_u8 => {
            println!("Error getting target floor");
            return Some(last_floor);
        }
    }
}

// Does what it says on the tin. Handles cab orders.
fn handle_cab_order (call_button: CallButton, last_floor: u8, elevator: Elevator, internal_order_channel_tx: Sender<InternalCommunication>) -> () {
    if call_button.floor < last_floor {
        let new_order = Order {
            floor_number: call_button.floor,
            direction: e::HALL_DOWN
        };
        let new_comm = InternalCommunication {
            intention: INSERT,
            order: Some(new_order)
        };
        internal_order_channel_tx.send(new_comm).unwrap();
        elevator.call_button_light(call_button.floor, call_button.call, true);
    }
    else if call_button.floor >= last_floor {
        let new_order = Order {
            floor_number: call_button.floor,
            direction: e::HALL_UP
        };
        let new_comm = InternalCommunication {
            intention: INSERT,
            order: Some(new_order)
        };
        internal_order_channel_tx.send(new_comm).unwrap();
        elevator.call_button_light(call_button.floor, call_button.call, true);
    }  
}

// Check if we need to stop
fn check_for_stop(floor: u8, mut dirn: u8, destination_list: HashSet<Order>, elevator: Elevator, last_floor: u8, elev_num_floors: u8, target_floor: u8, internal_order_channel_tx: Sender<InternalCommunication>, elevator_controller_tx: Sender<u8>) -> () {
    let mut destination_list_copy = destination_list.clone();
    for destination in destination_list_copy {
        if destination.floor_number == floor {
            if halldirn_to_elevdirn(destination.direction) == dirn {
                elevator_controller_tx.send(DIRN_STOP_TEMP).unwrap();
                println!("Stopper");

                let new_comm = InternalCommunication {
                    intention: DELETE,
                    order: Some(destination)
                };
                internal_order_channel_tx.send(new_comm).unwrap();

                elevator.door_light(true);
                sleep(Duration::from_millis(3000));
                elevator.door_light(false);
            }
            if destination.direction != dirn && floor == target_floor {
                elevator_controller_tx.send(DIRN_STOP_TEMP).unwrap();
                println!("Stopper");

                let new_comm = InternalCommunication {
                    intention: DELETE,
                    order: Some(destination)
                };
                internal_order_channel_tx.send(new_comm).unwrap();

                elevator.door_light(true);
                sleep(Duration::from_millis(3000));
                elevator.door_light(false);
            }
        }
    }
}

// Check if we're at the bottom of the lift
fn check_for_bottom(mut dirn: u8, floor: u8, elevator: Elevator, elev_num_floors: u8, elevator_controller_tx: Sender<u8>) -> () {
    println!("Retning: {:#?}", dirn);
    println!("Etasje: {:#?}", floor);
    if dirn == e::DIRN_UP && floor == (elev_num_floors-1)  {
        println!("Sender stopp");
        elevator_controller_tx.send(e::DIRN_STOP).unwrap();
    }
    else if dirn == e::DIRN_DOWN && floor == 0 {
        println!("Sender stopp");
        elevator_controller_tx.send(e::DIRN_STOP).unwrap();
    }
}

// Check whether or not to continue
fn continue_or_not(mut dirn: u8, floor: u8, target_floor: u8, elevator: Elevator, elevator_controller_tx: Sender<u8>) -> () {
    if (dirn == e::DIRN_UP && floor < target_floor) || (dirn == e::DIRN_DOWN && floor > target_floor) {
        elevator_controller_tx.send(dirn).unwrap();
    }
    else {
        dirn = e::DIRN_STOP;
        elevator_controller_tx.send(dirn).unwrap();
    }
}

fn floor_recieved(floor: u8, mut last_floor: u8, elevator: Elevator, elev_num_floors: u8, mut target_floor: u8, internal_order_channel_tx: Sender<InternalCommunication>, elevator_controller_tx: Sender<u8>, elevator_readout_rx: Receiver<u8>, destination_list_rx: Receiver<HashSet<Order>>) -> () {
                println!("Floor: {:#?}", floor);
                let new_comm = InternalCommunication {
                    intention: REQUEST_DESTINATION,
                    order: None
                };
                internal_order_channel_tx.send(new_comm).unwrap();
                let a = destination_list_rx.recv();
                let destination_list = a.unwrap();

                let new_comm2 = InternalCommunication {
                    intention: REQUEST_DIRECTION,
                    order: None
                };
                internal_order_channel_tx.send(new_comm2).unwrap();
                let dirn: u8 = elevator_readout_rx.recv().unwrap();
                println!("Mottat retning: {:#?}", dirn);
                //println!("Last floor updated to: {:#?}", last_floor);
                {
                let elevator = elevator.clone();
                let elevator_controller_tx = elevator_controller_tx.clone();
                check_for_bottom(dirn, floor, elevator, elev_num_floors, elevator_controller_tx);
                }
                
                if !destination_list.is_empty(){
                    let elevator = elevator.clone();
                    let destination_list_copy = destination_list.clone();
                    target_floor = target_floor_function(dirn, &destination_list_copy, last_floor).unwrap();
                    let elevator_controller_tx = elevator_controller_tx.clone();
                    check_for_stop(floor, dirn, destination_list_copy, elevator, last_floor, elev_num_floors, target_floor, internal_order_channel_tx, elevator_controller_tx);
                    }
                {
                let elevator = elevator.clone();
                let elevator_controller_tx = elevator_controller_tx.clone();
                continue_or_not(dirn, floor, target_floor, elevator, elevator_controller_tx);
                }
                check_lights(&elevator, dirn, floor, elev_num_floors);
}

fn elevator_memory(internal_order_channel_rx: Receiver<InternalCommunication>, destination_list_tx: Sender<HashSet<Order>>, elevator_readout_tx: Sender<u8>) -> () {
    let mut destination_list: HashSet<Order> = HashSet::new();
    let mut direction: u8 = e::DIRN_DOWN;
    loop {
        cbc::select! {
            recv(internal_order_channel_rx) -> a => {
                let communication = a.unwrap();
                match communication.intention {
                    INSERT => { // add
                        destination_list.insert(communication.order.unwrap());
                    }
                    DELETE => { // remove
                        destination_list.remove(&communication.order.unwrap());
                    }
                    REQUEST_DESTINATION => {
                        let destination_list_copy = destination_list.clone();
                        destination_list_tx.send(destination_list_copy).unwrap();
                    }
                    REQUEST_DIRECTION => {
                        elevator_readout_tx.send(direction).unwrap();
                        // println!("Retning sendt: {:#?}", direction);
                    }
                    UPDATE_DIRECTION => {
                        let order = communication.order.unwrap();
                        direction = order.direction;
                    }
                    2_u8..=5_u8|9_u8..=u8::MAX => {
                        println!("Wrong message to memory")
                    }
                }
            }
            default(Duration::from_millis(100)) => {
                //Chiller
            }
        }
    }
}

fn elevator_controller(elevator_controller_rx: Receiver<u8>, elevator: Elevator, internal_order_channel_tx: Sender<InternalCommunication>) -> () {
    let mut direction: u8 = e::DIRN_DOWN;
    loop {
        cbc::select! {
            recv(elevator_controller_rx) -> a => {
                let direction_ordered = a.unwrap();
                // println!("Mottat melding: {:#?}", direction_ordered);
                match direction_ordered {
                    e::DIRN_DOWN|e::DIRN_STOP|e::DIRN_UP => {
                        direction = direction_ordered;
                        elevator.motor_direction(direction);
                        println!("Retning satt");
                        let new_order = Order {
                            floor_number: 0,
                            direction: direction
                        };
                        let new_comm = InternalCommunication {
                            intention: UPDATE_DIRECTION,
                            order: Some(new_order)
                        };
                        internal_order_channel_tx.send(new_comm).unwrap();
                    }
                    DIRN_STOP_TEMP => {
                        elevator.motor_direction(e::DIRN_STOP);
                        sleep(Duration::from_millis(3000));
                        elevator.motor_direction(direction);
                    }
                    2_u8|4_u8..=254_u8 => {
                        println!("Feil ordre mottat i kontroller");
                    }
                }

            }
        }
    }
}

fn message_from_master(message: Communication, internal_order_channel_tx: Sender<InternalCommunication>, comms_channel_tx: Sender<Communication>) -> () {
    match message.comm_type {
        STATUS_MESSAGE => {
            // Message is not for me
        }
        ORDER_TRANSFER => {
            let new_order = message.order.unwrap();
            let new_comm = InternalCommunication {
                intention: INSERT,
                order: Some(new_order)
            };
            let internal_order_channel_tx = internal_order_channel_tx.clone();
            internal_order_channel_tx.send(new_comm).unwrap();

            let mut new_message = message;
            new_message.target = new_message.sender;
            new_message.sender = 0;
            new_message.comm_type = ORDER_ACK;
            comms_channel_tx.send(new_message).unwrap();
            sleep(Duration::from_millis(10));
        }
        ORDER_ACK => {
            // Message is not for me
        }
        3_u8..=u8::MAX => {
            println!("Feil i meldingssending")
        }
    }
}

// Elevator function. Runs forever (or till it panics)
pub fn run_elevator(elev_num_floors: u8, elevator: Elevator, poll_period: Duration, comms_channel_tx: Sender<Communication>, comms_channel_rx: Receiver<Communication>) -> () {

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
    //let mut dirn = e::DIRN_DOWN; 
    // Send the elevator down upon startup
    elevator.motor_direction(e::DIRN_DOWN);
    println!("På vei ned");
    
    // Set up variable to remember what floor we were last at
    let mut last_floor: u8 = elev_num_floors+1;
    let mut last_last_floor: u8 = 0;
    let mut target_floor: u8 = 0;

    // Setting up destination set with Rwlock
    // Rwlock means that it can either be written to by a single thread or read by any number of threads at once
    //let mut destination_list:RwLock<HashSet<Order>> = RwLock::from(HashSet::new());
    let mut last_destination_list: HashSet<Order> = HashSet::new();

    let (internal_order_channel_tx, internal_order_channel_rx) = cbc::unbounded();
    let (destination_list_tx, destination_list_rx) = cbc::unbounded();

    let (elevator_controller_tx, elevator_controller_rx) = cbc::unbounded();
    let (elevator_readout_tx, elevator_readout_rx) = cbc::unbounded::<u8>();

    {
    let elevator_readout_tx = elevator_readout_tx.clone();
    spawn(move || elevator_memory(internal_order_channel_rx, destination_list_tx, elevator_readout_tx));
    }

    

    {
    let elevator = elevator.clone();
    let elevator_controller_rx = elevator_controller_rx.clone();
    let internal_order_channel_tx = internal_order_channel_tx.clone();
    spawn(move || elevator_controller(elevator_controller_rx, elevator, internal_order_channel_tx));
    }

    // The main running loop of the elevator
    loop {
        // Crossbeam channel runs the main functions of the master
        // It constantly checks whether it has received a message and runs a standard function if it has waited too long
        cbc::select! {
            // Get info from call button and add it to the destination list if it is a cab call
            recv(call_button_rx) -> a => { 
                let call_button = a.unwrap();
                if call_button.call == e::CAB {
                    let elevator = elevator.clone();
                    let internal_order_channel_tx = internal_order_channel_tx.clone();
                    spawn(move||handle_cab_order(call_button, last_floor, elevator, internal_order_channel_tx));
                }
            }
            // Get floor status and save last floor for later use
            recv(floor_sensor_rx) -> a => {
                let floor = a.unwrap();
                last_floor = floor;
                println!("{:#?}", floor);
                {
                let elevator = elevator.clone();
                let internal_order_channel_tx = internal_order_channel_tx.clone();
                let elevator_controller_tx = elevator_controller_tx.clone();
                let elevator_readout_rx = elevator_readout_rx.clone();
                let destination_list_rx = destination_list_rx.clone();
                spawn(move || floor_recieved(floor, last_floor, elevator, elev_num_floors, target_floor, internal_order_channel_tx, elevator_controller_tx, elevator_readout_rx, destination_list_rx));
                }
            }
            // Get info from comms_channel and process according to status if it is meant for us
            recv(comms_channel_rx) -> a => {
                let message = a.unwrap();
                if message.target == 0 {
                    let internal_order_channel_tx = internal_order_channel_tx.clone();
                    let comms_channel_tx = comms_channel_tx.clone();
                    spawn (move || message_from_master(message, internal_order_channel_tx, comms_channel_tx));
                }
            }
            // This function polls continuously
            default(Duration::from_millis(100)) => {

                let new_comm2 = InternalCommunication {
                    intention: REQUEST_DIRECTION,
                    order: None
                };
                internal_order_channel_tx.send(new_comm2).unwrap();
                let direction = elevator_readout_rx.recv().unwrap();
                if direction == e::DIRN_STOP {
                    let new_comm = InternalCommunication {
                        intention: REQUEST_DESTINATION,
                        order: None
                    };
                    internal_order_channel_tx.send(new_comm).unwrap();
                    let a = destination_list_rx.recv();
                    let destination_list_copy = a.unwrap();

                    if !destination_list_copy.is_empty() {
                        for destination in &destination_list_copy{
                            if destination.floor_number > last_floor {
                                elevator_controller_tx.send(e::DIRN_UP).unwrap();
                                println!("Kjører");
                                break;
                            }
                            if destination.floor_number < last_floor {
                                elevator_controller_tx.send(e::DIRN_DOWN).unwrap();
                                println!("Kjører");
                                break;
                            }
                        }
                    }
                };
                    

                
                let new_comm = InternalCommunication {
                    intention: REQUEST_DESTINATION,
                    order: None
                };
                internal_order_channel_tx.send(new_comm).unwrap();
                let a = destination_list_rx.recv();
                let destination_list_copy = a.unwrap();
                // println!("{:#?}", destination_list_r);
                // Create and send status to master
                let current_status = Status {
                    last_floor: last_floor,
                    direction: direction,
                    errors: false,
                    obstructions: false,
                    target_floor: target_floor_function(direction, &destination_list_copy, last_floor)
                };
                
                let new_message = Communication {
                    sender: 0,
                    target: u8::MAX,
                    comm_type: STATUS_MESSAGE,
                    status: Some(current_status),
                    order: None
                };
                comms_channel_tx.send(new_message).unwrap();
                
                // Status update readout, mostly for debugging
                let mut destinations_up: HashSet<u8> = HashSet::new();
                let mut destinations_down: HashSet<u8> = HashSet::new();

                for element in &destination_list_copy {
                    if element.direction == e::HALL_UP {
                        destinations_up.insert(element.floor_number);
                    }
                    else if element.direction == e::HALL_DOWN {
                        destinations_down.insert(element.floor_number);
                    }
                }
                
                if destination_list_copy != last_destination_list || last_floor != last_last_floor {
                    last_destination_list = destination_list_copy.clone();
                    last_last_floor = last_floor.clone();
                    clearscreen::clear().unwrap();
                    let table = vec![
                    vec!["Etasje".cell(), last_floor.cell().justify(Justify::Right)],
                    vec!["Retning".cell(), direction_to_string(direction).cell().justify(Justify::Right)],
                    vec!["Destinasjoner opp".cell(), format!("{:#?}", destinations_up.clone()).cell().justify(Justify::Right)],
                    vec!["Destinasjoner ned".cell(), format!("{:#?}", destinations_down.clone()).cell().justify(Justify::Right)],
                    ]
                    .table()
                    .title(vec![
                        "Variabel".cell().bold(true),
                        "Verdi".cell().bold(true),
                    ])
                    .bold(true);

                    assert!(print_stdout(table).is_ok());
                }
            }
        }
    }
}