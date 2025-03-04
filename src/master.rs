use crate::prelude::*;

// Finds the relative distance to an order based on the current target floor.
fn cost_of_order(order: Order, status: Status) -> u8 {
    let target_floor = i32::from(status.target_floor.unwrap()); //
    let last_floor = i32::from(status.last_floor);
    let order_floor = i32::from(order.floor_number);
    let cost:i32;
    
    // Finds whether the order floor is between last_floor and target_floor, 
    // also checks if the order direction is the same as the elevator direction.
    if (order.direction==DIRN_UP && (target_floor>order_floor && order_floor>last_floor))
    || (order.direction==DIRN_DOWN && (target_floor<order_floor && order_floor<last_floor)) {
        // If the order floor is on our path, the cost is the distance between last_floor and order_floor
        cost = i32::abs(last_floor - order_floor);
    } else {
        // If the order floor is not on our path, the cost is the distance from last_floor to target_floor to order_floor
        cost = i32::abs(last_floor - target_floor) + i32::abs(target_floor - order_floor);
    }
    return cost as u8;
}

// Sends orders to the elevator
fn order_up(comms_channel_tx: Sender<Communication>, order_list: HashSet<Order>, status_list: Vec<Status>) -> () {
    let mut cost_of_orders = Vec::new();
    let mut status_list_copy = status_list.clone();
    for element in &order_list {
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
        println!("Sending order: {:#?}", new_message.order);
        comms_channel_tx.send(new_message).unwrap();
    }
}

fn add_hall_call(internal_order_channel_tx:Sender<InternalCommunication>, call_button:CallButton, elevator:Elevator)-> () {
    let new_order = Order {
        floor_number: call_button.floor,
        direction: call_button.call
    };
    let new_comm = InternalCommunication {
        intention: INSERT,
        order: Some(new_order)
    };
    internal_order_channel_tx.send(new_comm);
    elevator.call_button_light(call_button.floor, call_button.call, true);
}

fn receive_message(internal_order_channel_tx:Sender<InternalCommunication>, message: Communication, mut status_list_w: RwLockWriteGuard<'_, Vec<Status>>) -> () {
    if message.target == u8::MAX {
        match message.comm_type {
            STATUS_MESSAGE => {
                // println!("Received status: {:#?}", message.status);
                status_list_w[message.sender as usize] = message.status.unwrap();
            }
            ORDER_TRANSFER => {
                // Message is not for me
            }
            ORDER_ACK => {
                let new_comm = InternalCommunication {
                    intention: DELETE,
                    order: message.order
                };
                internal_order_channel_tx.send(new_comm);
            }
            3_u8..=u8::MAX => {
                println!("Feil i meldingssending")
            }
        }
    }
}

fn order_memory(internal_order_channel_rx: Receiver<InternalCommunication>, order_list_tx: Sender<HashSet<Order>>) -> () {
    let mut order_list: HashSet<Order> = HashSet::new();
    loop {
        cbc::select! {
            recv(internal_order_channel_rx) -> a => {
                let communication = a.unwrap();
                match communication.intention {
                    INSERT => { // add
                        order_list.insert(communication.order.unwrap());
                    }
                    DELETE => { // remove
                        order_list.remove(&communication.order.unwrap());
                    }
                    REQUEST_ORDER => {
                        let order_list_copy = order_list.clone();
                        order_list_tx.send(order_list_copy).unwrap();
                    }
                    2_u8..=5_u8|7_u8..=u8::MAX => {
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

// Master function. Runs forever (or till it panics)
pub fn run_master(elev_num_floors: u8, elevator: Elevator, poll_period: Duration, comms_channel_tx: Sender<Communication>, comms_channel_rx: Receiver<Communication>) -> () {

    let (call_button_tx, call_button_rx) = cbc::unbounded::<elevio::poll::CallButton>(); // Initialize call buttons
    {
        let elevator = elevator.clone();
        spawn(move || elevio::poll::call_buttons(elevator, call_button_tx, poll_period));
    }

    // Setting up prder set and status list with Rwlock
    // Rwlock means that it can either be written to by a single thread or read by any number of threads at once
    let mut status_list = RwLock::from(Vec::from([Status::new()]));

    let (internal_order_channel_tx, internal_order_channel_rx) = cbc::unbounded();
    let (order_list_tx, order_list_rx) = cbc::unbounded();

    { // spawn order memory
    spawn(move || order_memory(internal_order_channel_rx, order_list_tx));
    }

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
                    let elevator = elevator.clone();
                    let internal_order_channel_tx = internal_order_channel_tx.clone();
                    add_hall_call(internal_order_channel_tx, call_button, elevator); // Adds new hall call to order_list
                }
            }

            // Get info from comms_channel and process according to status if it is meant for us
            recv(comms_channel_rx) -> a => {
                let message = a.unwrap();
                // println!("Received message: {:#?}", message.comm_type);
                let status_list_w = status_list.write().unwrap();
                let internal_order_channel_tx = internal_order_channel_tx.clone();
                receive_message(internal_order_channel_tx, message, status_list_w);
            }
            // This function polls continuously if no other functions have been called
            default(Duration::from_millis(5)) => {
                // Opening status list for reading
                let status_list_r = status_list.read().unwrap();

                // If status has been received, ie. elevator is alive, try to send orders
                if status_list_r[0 as usize].direction != u8::MAX {
                    // Requesting order list from order memory
                    let request = InternalCommunication {
                        intention: REQUEST_ORDER,
                        order: None
                    };
                    internal_order_channel_tx.send(request);
                    let order_list = order_list_rx.recv().unwrap();
                    
                    let status_list_r_copy = status_list_r.clone();
                    let comms_channel_tx = comms_channel_tx.clone();
                    // Calling ordering function
                    order_up(comms_channel_tx, order_list, status_list_r_copy);
                }
                // println!("{:#?}", status_list);
            }
        }
    }
} 
