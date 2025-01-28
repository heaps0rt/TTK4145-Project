use std::thread::*;
use std::time::*;
use std::collections::HashSet;
use std::u8;

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

    let mut last_floor:u8 = elev_num_floors+1;

    let direction:i8 = -1; // Måte å huske retningen, ikke pen atm. Opp er 1, ned er -1 og stopp er 0. Denne må endres hver gang retning endres

    let mut ordered_floors = HashSet::new();


    loop {
        cbc::select! {
            recv(call_button_rx) -> a => { // Get info from call button and add it to the list of floors ordered
                let call_button = a.unwrap();
                println!("{:#?}", call_button);
                elevator.call_button_light(call_button.floor, call_button.call, true);

                // DISGUSTING nested ifs to add floor correctly
                if direction == -1 {
                    if call_button.floor < last_floor {
                        ordered_floors.insert(call_button.floor);
                    }
                }
                if direction == 1 {
                    if call_button.floor > last_floor {
                        ordered_floors.insert(call_button.floor);
                    }
                }
                if direction == 0 {
                    ordered_floors.insert(call_button.floor);
                }
                
            }
            recv(floor_sensor_rx) -> a => { // Get floor status and save last floor for later use
                let floor = a.unwrap();
                println!("Floor: {:#?}", floor);
                if elevator.floor_sensor().is_some() {
                    last_floor = floor;
                    println!("Last floor updated to: {:#?}", last_floor);
                }
                if floor == 0 {
                    dirn = e::DIRN_STOP;
                    elevator.motor_direction(dirn);
                }
            }
        }


    }
}
