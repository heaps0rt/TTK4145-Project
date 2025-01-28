use std::thread::*;
use std::time::*;
use std::collections::HashSet;

use crossbeam_channel as cbc;

use driver_rust::elevio;
use driver_rust::elevio::elev as e;

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
    if elevator.floor_sensor().is_none() { // If the elevator is not on a floor, send it down
        elevator.motor_direction(dirn);
    }

    let mut lastFloor = none;

    let mut orderedFloors = HashSet::new();


    loop {
        cbc::select! {
            recv(call_button_rx) -> a => { // Get info from call button and add it to the list of floors ordered
                let call_button = a.unwrap();
                println!("{:#?}", call_button);
                elevator.call_button_light(call_button.floor, call_button.call, true);
                orderedFloors.insert(call_button.floor)
            }
            recv(floor_sensor_rx) -> a => { // Get floor status and save last floor for later use
                let floor = a.unwrap();
                println!("Floor: {:#?}", floor);
                if elevator.floor_sensor().is_none().not() {
                    lastFloor = floor;
                }
            }
        }


    }
}