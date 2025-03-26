pub use std::default;
pub use std::hash::Hash;
pub use std::thread::*;
pub use std::time::*;
pub use std::collections::HashSet;
pub use std::u8;
pub use std::sync::*;
pub use std::cmp::max;
pub use std::ops::Not;
pub use serde::{Serialize,Deserialize};

pub use crossbeam_channel::Receiver;
pub use crossbeam_channel::Sender;
pub use crossbeam_channel as cbc;

pub use driver_rust::elevio;
pub use driver_rust::elevio::elev::Elevator;
pub use driver_rust::elevio::elev::DIRN_DOWN;
pub use driver_rust::elevio::elev::DIRN_STOP;
pub use driver_rust::elevio::elev::DIRN_UP;
pub use driver_rust::elevio::elev::HALL_DOWN;
pub use driver_rust::elevio::elev::HALL_UP;
pub use driver_rust::elevio::elev as e;
pub use driver_rust::elevio::poll;
pub use driver_rust::elevio::poll::floor_sensor;
pub use driver_rust::elevio::poll::CallButton;

// Libraries we have added go below
pub use cli_table::{format::Justify, print_stdout, Cell, Style, Table};
pub use clearscreen;

// Structure for a hall order
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

// Print an order for testing purposes
pub fn print_order(order: &Order) -> () {
    let floor = order.floor_number;
    let direction = order.direction;
    println!("Floor: \n{:#?}", floor);
    println!("Direction: \n{:#?}", direction);

}

// Structure for the status of an elevator
#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, PartialOrd, Serialize, Deserialize)]
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

// Structure for cross-module communication. (Eventually replaced by networking)
#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub struct Communication {
    pub sender: u8,
    pub target: u8,
    pub comm_type: u8,
    pub status: Option<Status>,
    pub order: Option<Order>
}

// Const variables for use in comms
pub const STATUS_MESSAGE: u8 = 0;
pub const ORDER_TRANSFER: u8 = 1;
pub const ORDER_ACK: u8 = 2;

// Structure for internal communications through message passing
#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub struct InternalCommunication {
    pub intention: u8, // use code words defined below
    pub order: Option<Order>
}

// Const variables for use in internal comms
pub const DELETE: u8 = 0;
pub const INSERT: u8 = 1;
pub const REQUEST_DESTINATION: u8 = 6;
pub const REQUEST_ORDER: u8 = 6;
pub const REQUEST_DIRECTION: u8 = 7;
pub const UPDATE_DIRECTION: u8 = 8;

pub const DIRN_STOP_TEMP: u8 = 3;