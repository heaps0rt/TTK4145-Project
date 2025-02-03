use std::default;
use std::thread::*;
use std::time::*;
use std::collections::HashSet;
use std::u8;
use std::sync::*;

use crossbeam_channel::Receiver;
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

// The big ordering function. Turns orders on the order list into destinations for the elevator.
fn commute_orders(order_list: &HashSet<Order>, destination_list: &HashSet<u8>, last_floor: u8, dirn: &u8, elevator: &Elevator, elev_num_floors: u8) -> (HashSet<Order>, HashSet<u8>, u8) {
    // Copying variables to keep owner control happy
    let mut order_list_copy = order_list.clone();
    let mut destination_list_copy = destination_list.clone();
    let mut dirn_copy = dirn.clone();
    // Creating a set to remove later
    let mut orders_to_remove = HashSet::new();
    for element in &order_list_copy {
        match dirn_copy {
            e::DIRN_UP => {
                match element.direction {
                    e::HALL_UP|e::CAB => {
                        if element.floor_number > last_floor {
                            // Adding to destinations and scheduling removal from orders
                            destination_list_copy.insert(element.floor_number);
                            orders_to_remove.insert(element);
                        }
                    }
                    e::HALL_DOWN => {
                        if element.floor_number == (elev_num_floors-1) {
                            // Adding to destinations and scheduling removal from orders
                            destination_list_copy.insert(element.floor_number);
                            orders_to_remove.insert(element);
                        }
                    }
                    3_u8..=u8::MAX => {
                        println!("Noe har gått kraftig galt");
                    }
                }
            }
            e::DIRN_DOWN => {
                match element.direction {
                    e::HALL_DOWN|e::CAB => {
                        if element.floor_number < last_floor {
                            // Adding to destinations and scheduling removal from orders
                            destination_list_copy.insert(element.floor_number);
                            orders_to_remove.insert(element);
                        }
                    }
                    e::HALL_UP => {
                        if element.floor_number == 0 {
                            // Adding to destinations and scheduling removal from orders
                            destination_list_copy.insert(element.floor_number);
                            orders_to_remove.insert(element);
                        }
                    }
                    3_u8..=u8::MAX => {
                        println!("Noe har gått kraftig galt");
                    }
                }
            }
            e::DIRN_STOP => {
                // Adding to destinations and scheduling removal from orders
                destination_list_copy.insert(element.floor_number);
                orders_to_remove.insert(element);
                // Restarting elevator
                if element.floor_number < last_floor {
                    dirn_copy = e::DIRN_DOWN;
                    elevator.motor_direction(dirn_copy);
                }
                else if element.floor_number > last_floor {
                    dirn_copy = e::DIRN_UP;
                    elevator.motor_direction(dirn_copy);
                }
                break; // Breaking out so only one order is added
            }
            2_u8..=254_u8 => {
                println!("Noe har gått kraftig galt");
            }
        }
        // Removing unneccesary orders
        if element.floor_number == last_floor {
            match element.direction {
                e::HALL_DOWN => {
                    if dirn_copy == e::DIRN_DOWN {
                        // Scheduling deletion of order
                        orders_to_remove.insert(element);
                        elevator.call_button_light(last_floor, e::HALL_DOWN, false);
                    }
                }
                e::HALL_UP => {
                    if dirn_copy == e::DIRN_UP {
                        // Scheduling deletion of order
                        orders_to_remove.insert(element);
                        elevator.call_button_light(last_floor, e::HALL_UP, false);
                    }
                }
                e::CAB => {
                    // Scheduling deletion of order
                    orders_to_remove.insert(element);
                    elevator.call_button_light(last_floor, e::CAB, false);
                }
                3_u8..=u8::MAX => {
                    println!("Noe har gått kraftig galt");
                }
            }
        }
    }
    // Copying a copy to be able to use it in the for loop
    let mut order_list_copy_copy = order_list_copy.clone();
    // Removing scheduled orders from the order list we will return
    for element in &orders_to_remove {
        order_list_copy_copy.remove(element);
    }

    // Returning values for further use
    return (order_list_copy_copy, destination_list_copy, dirn_copy);

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

fn runMaster(elev_num_floors: u8, elevator: Elevator, poll_period: Duration) -> () {

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
    let mut status_list = RwLock::from(HashSet::from([0]));
    let mut elevator_direction = e::DIRN_STOP;

    loop {
        cbc::select! {
            recv(call_button_rx) -> a => { // Get info from call button and add it to the list of floors ordered if it is a hall call
                let call_button = a.unwrap();
                if call_button.floor == e::HALL_DOWN | HALL_UP {
                    let new_order = Order {
                        floor_number: call_button.floor,
                        direction: call_button.call,
                        status: WAITING,
                    };
                    let mut order_list_w = order_list.write().unwrap();
                    order_list_w.insert(new_order);
                    elevator.call_button_light(call_button.floor, call_button.call, true);

                }
        
            }
            // This function polls continuously
            default(a_hundred_millis) => {
                // Status update readout
                let status_list_w = status_list.write().unwrap();
                println!("-------Status---------");
                println!("Destinasjoner: {:#?}", status_list_w);
                match elevator_direction{
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

fn runElevator(elev_num_floors: u8, elevator: Elevator, poll_period: Duration) -> () {

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
                if call_button.floor == e::CAB {
                    match dirn {
                        e::DIRN_DOWN => {
                            if call_button.floor < last_floor {
                                let mut destination_list_w = destination_list.write().unwrap();
                                destination_list_w.insert(call_button.floor);
                            }
                        }
                        e::DIRN_UP => {
                            if call_button.floor > last_floor {
                                let mut destination_list_w = destination_list.write().unwrap();
                                destination_list_w.insert(call_button.floor);
                            }
                        }
                        e::DIRN_STOP => {
                            let mut destination_list_w = destination_list.write().unwrap();
                            destination_list_w.insert(call_button.floor);
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
                    if !destination_list_w.is_empty() {
                        dirn = e::DIRN_DOWN;
                        elevator.motor_direction(dirn);
                    }
                }
                else if dirn == e::DIRN_DOWN && floor == 0 {
                    dirn = e::DIRN_STOP;
                    elevator.motor_direction(dirn);
                    if !destination_list_w.is_empty() {
                        dirn = e::DIRN_UP;
                        elevator.motor_direction(dirn);
                    }
                }
            }
            // This function polls continuously
            default(a_hundred_millis) => {

            }
        }
    }
}

fn main() -> std::io::Result<()> {
    

    let elev_num_floors = 4; // Set floor count
    let elevator = e::Elevator::init("localhost:15657", elev_num_floors)?; // Initialize and connect elevator
    println!("Elevator started:\n{:#?}", elevator);

    let poll_period = Duration::from_millis(25); // Set poll period for buttons and sensors
    {
    let elevator = elevator.clone();
    let master = spawn(move || {
        runMaster(elev_num_floors, elevator, poll_period);
    });
    }

    {
    let elevator = elevator.clone();
    let elevator1 = spawn(move || {
        runElevator(elev_num_floors, elevator, poll_period);
    });
    }  
    

    /* loop {
        cbc::select! {
            // This function polls continuously
            default(a_hundred_millis) => {
                //  Unlocking Rw locked lists
                let mut order_list_w = order_list.write().unwrap();
                let mut destination_list_w = destination_list.write().unwrap();

                // Calling order scheduler
                let out = commute_orders(&order_list_w, &destination_list_w, last_floor, &dirn, &elevator, elev_num_floors);

                // Overwriting old variables with new modified ones
                *order_list_w = out.0;
                *destination_list_w = out.1;
                dirn = out.2;

                
            }
            
        }

    } */
   loop {

   }
}
