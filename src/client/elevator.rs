use crate::prelude::*;
use crate::client::utils::*;

// When a new foor is passed checks whether we should stop and open the door, then checks whether we should continue
fn floor_recieved(
    floor: u8,
    last_floor: u8,
    elevator: Elevator,
    elev_num_floors: u8,
    internal_order_channel_tx: Sender<InternalCommunication>,
    elevator_controller_tx: Sender<u8>,
    elevator_readout_rx: Receiver<u8>,
    destination_list_rx: Receiver<HashSet<Order>>
) -> () {
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
                // println!("Mottat retning: {:#?}", dirn);
                // println!("Last floor updated to: {:#?}", last_floor);
                {
                let elevator_controller_tx = elevator_controller_tx.clone();
                check_for_bottom(dirn, floor, elev_num_floors, elevator_controller_tx);
                }
                
                // First we find our target floor
                let destination_list_copy = destination_list.clone();
                let target_floor = target_floor_function(dirn, destination_list_copy, last_floor).unwrap_or(floor);

                // Secondly we find our heading after the current floor; up, down, or stop
                let heading = check_continue_or_not(dirn, floor, target_floor);

                if !destination_list.is_empty() {
                    let destination_list = destination_list.clone();
                    let elevator_controller_tx = elevator_controller_tx.clone();
                    let internal_order_channel_tx = internal_order_channel_tx.clone();
                    if check_for_stop(floor, dirn, destination_list, target_floor, internal_order_channel_tx, elevator_controller_tx) {
                        // Open the door during temp_stop and check lights
                        elevator.door_light(true);
                        check_lights(&elevator, heading, floor, elev_num_floors);
                        sleep(Duration::from_millis(3000));
                        elevator.door_light(false);
                    }
                }
                elevator_controller_tx.send(heading).unwrap();
}

// Check if we're at the bottom of the elevator
fn check_for_bottom(dirn: u8, floor: u8, elev_num_floors: u8, elevator_controller_tx: Sender<u8>) -> () {
    println!("Retning: {:#?}", dirn);
    println!("Etasje: {:#?}", floor);
    if (dirn == e::DIRN_UP && floor == (elev_num_floors-1))
    || (dirn == e::DIRN_DOWN && floor == 0) {
        elevator_controller_tx.send(e::DIRN_STOP).unwrap();
        println!("CHECKFORBOTTOM stopping");
    }
}

// Returns the next heading
fn check_continue_or_not(dirn: u8, floor: u8, target_floor: u8) -> u8 {
    // print!("\nCHECK_CONTINUE\n");
    // print!("dirn  {:#?}\n",dirn);
    // print!("floor  {:#?}\n",floor);
    // print!("target_floor  {:#?}\n",target_floor);
    if (dirn == e::DIRN_UP && floor < target_floor) || (dirn == e::DIRN_DOWN && floor > target_floor) {
        return dirn;
    }
    println!("CHECKCONTINUE stopping");
    return e::DIRN_STOP;
}

// Check if we need to stop
fn check_for_stop(
    floor: u8,
    dirn: u8,
    destination_list: HashSet<Order>,
    target_floor: u8,
    internal_order_channel_tx: Sender<InternalCommunication>,
    elevator_controller_tx: Sender<u8>
) -> bool {
    for destination in destination_list {
        if destination.floor_number == floor {
            if (halldirn_to_elevdirn(destination.direction) == dirn) || (floor == target_floor) {
                elevator_controller_tx.send(DIRN_STOP_TEMP).unwrap();
                println!("CHECKFORSTOP stopping");

                let new_comm = InternalCommunication {
                    intention: DELETE,
                    order: Some(destination)
                };
                internal_order_channel_tx.send(new_comm).unwrap();

                return true;
            }
        }
    }
    return false;
}

// Turns off the correct lights based on the elevator floor and direction
fn check_lights(elevator: &Elevator, dirn: u8, floor: u8, num_floors: u8) -> () {
    elevator.call_button_light(floor, e::CAB, false);
    if (dirn == e::DIRN_DOWN) || (floor == (num_floors-1)) {
        elevator.call_button_light(floor, e::HALL_DOWN, false);
    }
    else if (dirn == e::DIRN_UP) || (floor == 0) {
        elevator.call_button_light(floor, e::HALL_UP, false);
    }
    else if dirn == e::DIRN_STOP {
        elevator.call_button_light(floor, e::HALL_DOWN, false);
        elevator.call_button_light(floor, e::HALL_UP, false);
    }
}

// Handles cab orders.
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

// Sends a hall call to the master
fn handle_hall_call(comms_channel_tx:Sender<Communication>, call_button:CallButton, elevator:Elevator)-> () {
    let new_order = Order {
        floor_number: call_button.floor,
        direction: call_button.call
    };
    let new_comm = Communication {
        sender: u8::MAX,
        sender_role: u8::MAX,
        target: MASTER,
        comm_type: ORDER_TRANSFER,
        status: None,
        order: Some(new_order)
    };
    comms_channel_tx.send(new_comm).unwrap();
    elevator.call_button_light(call_button.floor, call_button.call, true);
}

// Elevator memory that keeps a destination list and a direction for message passing
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
            // default(Duration::from_millis(100)) => {
            //     //Chiller
            // }
        }
    }
}

// Controls the direction of the elevator through the elevator_controller channel
fn handle_elevator_controller(elevator_controller_rx: Receiver<u8>, elevator: Elevator, internal_order_channel_tx: Sender<InternalCommunication>) -> () {
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
                        println!("Retning satt til {:#?}",direction_to_string(direction));
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
                        println!("TEMPSTOP");
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

// Handles external communications from master; recieves new orders from master
fn handle_message_from_master(message: Communication, internal_order_channel_tx: Sender<InternalCommunication>, comms_channel_tx: Sender<Communication>) -> () {
    println!("Recieved {:#?}", message);
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

fn send_elevator_startup(last_floor:u8,direction: u8,destination_list_copy: HashSet<Order>,elevator_controller_tx:Sender<u8>)->() {
    if direction == e::DIRN_STOP {
        if !destination_list_copy.is_empty() {
            for destination in destination_list_copy{
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
}

// Create and send status to master
fn send_status_update(last_floor:u8,direction: u8,destination_list: HashSet<Order>,comms_channel_tx:Sender<Communication>)->() {
    // println!("{:#?}", destination_list_r);
    
    let current_status = Status {
        last_floor: last_floor,
        direction: direction,
        errors: false,
        obstructions: false,
        target_floor: target_floor_function(direction, destination_list, last_floor)
    };
    
    let new_message = Communication {
        sender: u8::MAX,
        sender_role: u8::MAX,
        target: TARGET_ALL,
        comm_type: STATUS_MESSAGE,
        status: Some(current_status),
        order: None
    };
    comms_channel_tx.send(new_message).unwrap();
}

// Status update readout, mostly for debugging
fn readout_status(last_floor:u8,direction: u8,destination_list: HashSet<Order>,last_last_floor: &mut u8,last_destination_list: &mut HashSet<Order>)->() {
    let mut destinations_up: HashSet<u8> = HashSet::new();
    let mut destinations_down: HashSet<u8> = HashSet::new();

    for element in &destination_list {
        if element.direction == e::HALL_UP {
            destinations_up.insert(element.floor_number);
        }
        else if element.direction == e::HALL_DOWN {
            destinations_down.insert(element.floor_number);
        }
    }
    
    if destination_list != *last_destination_list || last_floor != *last_last_floor {
        *last_destination_list = destination_list.clone();
        *last_last_floor = last_floor.clone();
        // clearscreen::clear().unwrap();
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

// Elevator function. Runs forever (or till it panics)
pub fn run_elevator(id:u8,elev_num_floors: u8, elevator: Elevator, poll_period: Duration, comms_channel_tx: Sender<Communication>, comms_channel_rx: Receiver<Communication>) -> () {

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
    let (stop_button_tx, _stop_button_rx) = cbc::unbounded::<bool>(); 
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::stop_button(elevator, stop_button_tx, poll_period));
    }
    // Initialize obstruction switch
    let (obstruction_tx, _obstruction_rx) = cbc::unbounded::<bool>(); 
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

    // Setting up last_last variables for the purposes of readout function
    let mut last_destination_list: HashSet<Order> = HashSet::new();
    let mut last_last_floor: u8 = 0;

    let (internal_order_channel_tx, internal_order_channel_rx) = cbc::bounded(1);
    let (destination_list_tx, destination_list_rx) = cbc::bounded(1);

    let (elevator_controller_tx, elevator_controller_rx) = cbc::bounded(1);
    let (elevator_readout_tx, elevator_readout_rx) = cbc::bounded::<u8>(1);

    {
    let elevator_readout_tx = elevator_readout_tx.clone();
    spawn(move || elevator_memory(internal_order_channel_rx, destination_list_tx, elevator_readout_tx));
    }

    

    {
    let elevator = elevator.clone();
    let elevator_controller_rx = elevator_controller_rx.clone();
    let internal_order_channel_tx = internal_order_channel_tx.clone();
    spawn(move || handle_elevator_controller(elevator_controller_rx, elevator, internal_order_channel_tx));
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
                } else {
                    let elevator = elevator.clone();
                    let comms_channel_tx = comms_channel_tx.clone();
                    handle_hall_call(comms_channel_tx, call_button, elevator); // Sends new hall call to master
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
                spawn(move || floor_recieved(floor, last_floor, elevator, elev_num_floors, internal_order_channel_tx, elevator_controller_tx, elevator_readout_rx, destination_list_rx));
                }
            }
            // Get info from comms_channel and process according to status if it is meant for us
            recv(comms_channel_rx) -> a => {
                let message = a.unwrap();
                if message.target == id {
                    println!("Comms Recieved {:#?}", message);
                    let internal_order_channel_tx = internal_order_channel_tx.clone();
                    let comms_channel_tx = comms_channel_tx.clone();
                    spawn (move || handle_message_from_master(message, internal_order_channel_tx, comms_channel_tx));
                }
            }
            // This function polls continuously
            default(Duration::from_millis(1000)) => {
                let new_comm2 = InternalCommunication {
                    intention: REQUEST_DIRECTION,
                    order: None
                };
                internal_order_channel_tx.send(new_comm2).unwrap();
                let direction = elevator_readout_rx.recv().unwrap();
                
                let new_comm = InternalCommunication {
                    intention: REQUEST_DESTINATION,
                    order: None
                };
                internal_order_channel_tx.send(new_comm).unwrap();
                let a = destination_list_rx.recv();
                let destination_list = a.unwrap();
        
                {
                let destination_list_copy = destination_list.clone();
                let elevator_controller_tx = elevator_controller_tx.clone();
                send_elevator_startup(last_floor,direction,destination_list_copy,elevator_controller_tx);
                }
                {
                let destination_list_copy = destination_list.clone();
                let comms_channel_tx = comms_channel_tx.clone();
                send_status_update(last_floor,direction,destination_list_copy,comms_channel_tx);
                }
                {
                let destination_list_copy = destination_list.clone();
                readout_status(last_floor, direction, destination_list_copy,&mut last_last_floor,&mut last_destination_list);
                }
            }
        }
    }
}