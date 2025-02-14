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

fn print_order(order: &Order) -> () {
    let floor = order.floor_number;
    let direction = order.direction;
    println!("Floor: \n{:#?}", floor);
    println!("Direction: \n{:#?}", direction);

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

mod elevator;

fn cost_of_order(order: Order, status: Status) -> u8 {
    let target_floor = i32::from(status.target_floor.unwrap());
    let last_floor = i32::from(status.last_floor);
    let floor = i32::from(order.floor_number);
    let cost = i32::abs(last_floor - target_floor) + i32::abs(target_floor - floor);

    return cost as u8;
}

// Sends orders to the elevator
fn order_up(comms_channel_tx: Sender<Communication>, order_list_w_copy: HashSet<Order>, status_list: Vec<Status>) -> () {
    let mut cost_of_orders = Vec::new();
    let mut status_list_copy = status_list.clone();
    for element in &order_list_w_copy {
        for status in &status_list_copy {
            cost_of_orders.insert(cost_of_orders.len(), cost_of_order(*element, *status));
        }
        let max = cost_of_orders.iter().max().unwrap();
        let max_index = cost_of_orders.iter().position(|part| part == max).unwrap();
        let new_message = Communication {
            sender: u8::MAX,
            target: max_index as u8,
            comm_type: ORDER_TRANSFER,
            status: None,
            order: Some(*element)
        };
        // println!("Sending order: {:#?}", new_message.order);
        comms_channel_tx.send(new_message).unwrap();
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
    let mut status_list = RwLock::from(Vec::from([Status::new()]));
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
            default(Duration::from_millis(5)) => {
                // Opening status list for reading
                let mut status_list_r = status_list.read().unwrap();

                // If status has been received, ie. elevator is alive, try to send orders
                if status_list_r[0 as usize].direction != u8::MAX {
                    // Opening order list for reading and cloning it
                    // This should be done differently, order_list is hogged
                    let mut order_list_r = order_list.read().unwrap();
                    let mut order_list_r_copy = order_list_r.clone();
                    let mut status_list_r_copy = status_list_r.clone();
                    let comms_channel_tx = comms_channel_tx.clone();
                    // Calling ordering function
                    order_up(comms_channel_tx, order_list_r_copy, status_list_r_copy);
                }
                // println!("{:#?}", status_list);
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
    /* spawn(move || {
        run_master(elev_num_floors, elevator, poll_period, comms_channel_tx, comms_channel_rx);
    });*/
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
        elevator::run_elevator(elev_num_floors, elevator, poll_period, comms_channel_tx, comms_channel_rx);
    });
    }

    // Main thread just loops so program doesn't shut down
    // Error handling goes here eventually
    loop {
        sleep(five_hundred_millis);
        assert!(now.elapsed() >= five_hundred_millis);
   }
}
