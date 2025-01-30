use std::thread::*;
use std::time::*;
use std::collections::HashSet;
use std::u8;

use crossbeam_channel::Receiver;
use crossbeam_channel as cbc;

use driver_rust::elevio;
use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio::elev::HALL_DOWN;
use driver_rust::elevio::elev::HALL_UP;
use driver_rust::elevio::elev as e;
use driver_rust::elevio::poll::CallButton;

pub const WAITING: u8 = 0;
pub const SENT: u8 = 1;
pub const ACK: u8 = 2;

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct Order {
    pub floor_number: u8,
    pub direction: u8,
    pub status: u8,
}
fn print_order(order: &Order) -> () {
    let floor = order.floor_number;
    let direction = order.direction;
    let status = order.status;
    println!("Floor: \n{:#?}", floor);
    println!("Direction: \n{:#?}", direction);
    println!("Status: \n{:#?}", status);

}

fn main() -> std::io::Result<()> {
    let elev_num_floors = 4; // Set floor count
    let elevator = e::Elevator::init("localhost:15657", elev_num_floors)?; // Initialize and connect elevator
    println!("Elevator started:\n{:#?}", elevator);

    let poll_period = Duration::from_millis(25); // Set poll period for buttons and sensors

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

    let mut dirn = e::DIRN_DOWN; // Set mutex direction
    // If the elevator is not on a floor, send it down
    elevator.motor_direction(dirn);
    println!("På vei ned");
    

    let mut last_floor: u8 = elev_num_floors+1;

    let mut order_list = HashSet::new();
    let mut destination_list = HashSet::from([0]);


    loop {
        cbc::select! {
            recv(call_button_rx) -> a => { // Get info from call button and add it to the list of floors ordered
                let call_button = a.unwrap();
                println!("{:#?}", call_button);
                elevator.call_button_light(call_button.floor, call_button.call, true);
                let new_order = Order {
                    floor_number: call_button.floor,
                    direction: call_button.call,
                    status: WAITING,
                };

                order_list.insert(new_order);
                print_order(&new_order);
            
            }
        

            recv(floor_sensor_rx) -> a => { // Get floor status and save last floor for later use
                let floor = a.unwrap();
                println!("Floor: {:#?}", floor);
                
                if destination_list.contains(&floor){
                    elevator.motor_direction(e::DIRN_STOP);
                    println!("Stopper midlertidig");
                    destination_list.remove(&floor);
                    if destination_list.is_empty() {
                        dirn = e::DIRN_STOP;
                    }
                    else {
                        elevator.motor_direction(dirn);
                        println!("Fortsetter");
                    }
                }
                if elevator.floor_sensor().is_some() {
                    last_floor = floor;
                    println!("Last floor updated to: {:#?}", last_floor);
                    elevator.call_button_light(floor, e::CAB, false);
                    if dirn == e::DIRN_DOWN {
                        elevator.call_button_light(floor, e::HALL_DOWN, false);
                    }
                    else if dirn == e::DIRN_UP {
                        elevator.call_button_light(floor, e::HALL_UP, false);
                    }
                    else if dirn == e::DIRN_STOP {
                        elevator.call_button_light(floor, e::HALL_DOWN, false);
                        elevator.call_button_light(floor, e::HALL_UP, false);
                    }
                }

                
            }
        }
        let mut orders_added = HashSet::new();
        for element in &order_list {
            if (element.direction == 0 || element.direction == 2) && dirn == e::DIRN_UP && element.floor_number > last_floor {
                destination_list.insert(element.floor_number);
                let new_order = Order {
                    floor_number: element.floor_number,
                    direction: element.direction,
                    status: element.status,
                };
                orders_added.insert(new_order);
            }
            else if (element.direction == 1 || element.direction == 2) && dirn == e::DIRN_DOWN && element.floor_number < last_floor {
                destination_list.insert(element.floor_number);
                let new_order = Order {
                    floor_number: element.floor_number,
                    direction: element.direction,
                    status: element.status,
                };
                orders_added.insert(new_order);
            }
            else if dirn == e::DIRN_STOP && element.floor_number != last_floor {
                destination_list.insert(element.floor_number);
                let new_order = Order {
                    floor_number: element.floor_number,
                    direction: element.direction,
                    status: element.status,
                };
                orders_added.insert(new_order);
                if element.floor_number > last_floor {
                    dirn = e::DIRN_UP;
                    elevator.motor_direction(dirn);
                    println!("Kjører ned")
                }
                else if element.floor_number < last_floor {
                    dirn = e::DIRN_DOWN;
                    elevator.motor_direction(dirn);
                    println!("Kjører opp")
                }
            }
        }
        let almost_final_orders = order_list.difference(&orders_added).collect::<HashSet<_>>();
        let mut final_orders = HashSet::new();
        for element in &almost_final_orders {
            let new_order = Order {
                floor_number: element.floor_number,
                direction: element.direction,
                status: element.status,
            };
            final_orders.insert(new_order);
        }
        order_list = final_orders;
    }
}
