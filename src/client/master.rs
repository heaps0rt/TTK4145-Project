use crate::prelude::*;
use crate::network::server::*;

// Finds the relative distance to an order based on the current target floor.
fn cost_of_order(order: Order, status: Status) -> u8 {
    println!("Finding cost of {:#?}", order);
    println!("with {:#?}", status);

    let last_floor = i32::from(status.last_floor);
    let order_floor = i32::from(order.floor_number);

    if status.target_floor.is_none() {
        return i32::abs(last_floor - order_floor) as u8;
    }

    let target_floor = i32::from(status.target_floor.unwrap()); //
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
fn order_up(
    comms_channel_tx: Sender<Communication>,
    order_list: HashSet<Order>,
    state_list: HashSet<State>,
) -> () {
    let status_list: Vec<Status> = state_list.iter().map(|state| state.status).collect();

    let mut cost_of_orders = Vec::new();
    for order in &order_list {
        // Calculate costs for this order against all statuses
        cost_of_orders.clear();
        for status in &status_list {
            cost_of_orders.push(cost_of_order(*order, *status));
        }

        // Find the unit with minimum cost
        let min_cost = cost_of_orders.iter().min().unwrap();
        let (best_unit_index, _) = cost_of_orders.iter()
            .enumerate()
            .find(|(_, &cost)| cost == *min_cost)
            .unwrap();

        // Get the corresponding State to find the unit ID
        let best_unit_state = state_list.iter()
            .nth(best_unit_index)
            .unwrap();

        let new_message = Communication {
            sender: u8::MAX,  // System-generated message
            sender_role: u8::MAX,
            target: best_unit_state.id,  // Target the unit by its ID
            comm_type: ORDER_TRANSFER,
            status: None,
            order: Some(*order)
        };

        println!("Sending order to unit {}: {:?}", best_unit_state.id, new_message.order);
        comms_channel_tx.send(new_message).unwrap();
    }
}

// Recieves external communcations and processes based on the comm_type
fn receive_message(internal_order_channel_tx:Sender<InternalCommunication>, message: Communication) -> () {
    if message.target == u8::MAX {
        match message.comm_type {
            STATUS_MESSAGE => { // handled on the network unit
            }
            ORDER_TRANSFER => {
                // Message is not for me
            }
            ORDER_ACK => { // Sends message to order memory in order to delete acknowledged order.
                let new_comm = InternalCommunication {
                    intention: DELETE,
                    order: message.order
                };
                internal_order_channel_tx.send(new_comm).unwrap();
            }
            3_u8..=u8::MAX => {
                println!("Feil i meldingssending")
            }
        }
    }
}

// Order memory that keeps a list of orders to be edited and read through message passing.
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
            // default(Duration::from_millis(100)) => {
            //     //Chiller
            // }
        }
    }
}

// Master function. Runs forever (or till it panics)
pub fn run_master(network_unit:NetworkUnit,comms_channel_tx: Sender<Communication>, comms_channel_rx: Receiver<Communication>) -> () {

    // setting up internal memory channel
    let (internal_order_channel_tx, internal_order_channel_rx) = cbc::bounded(1);
    let (order_list_tx, order_list_rx) = cbc::bounded(1);

    { // spawn order memory
    spawn(move || order_memory(internal_order_channel_rx, order_list_tx));
    }

    // Main master loop
    loop {
        // Crossbeam channel runs the main functions of the master
        // It constantly checks whether it has received a message and runs a standard function if it has waited too long
        cbc::select! {
            // Get info from comms_channel and process according to status if it is meant for us
            recv(comms_channel_rx) -> a => {
                let message = a.unwrap();
                // println!("Received message: {:#?}", message.comm_type);
                let internal_order_channel_tx = internal_order_channel_tx.clone();
                receive_message(internal_order_channel_tx, message);
            }
            // This function polls continuously if no other functions have been called
            default(Duration::from_millis(500)) => {
                // Opening status list for reading
                let state_list = network_unit.get_state_list();

                // If status has been received, ie. elevator is alive, try to send orders
                if !state_list.is_empty() {
                    // Requesting order list from order memory
                    let request = InternalCommunication {
                        intention: REQUEST_ORDER,
                        order: None
                    };
                    internal_order_channel_tx.send(request).unwrap();
                    let order_list = order_list_rx.recv().unwrap();
                    
                    // Calling ordering function
                    if !order_list.is_empty() {
                        println!("Ordering up with order_list{:#?}", order_list);
                        let comms_channel_tx = comms_channel_tx.clone();
                        order_up(comms_channel_tx, order_list, state_list);
                    }
                }
                // println!("{:#?}", status_list);
            }
        }
    }
} 
