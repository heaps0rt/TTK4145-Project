use crate::prelude::*;

// Turns a direction const into a string (for testing)
pub fn direction_to_string(dirn: u8) -> String {
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

// Turns an elevator direction const into the corresponding hall direction const
pub fn elevdirn_to_halldirn(dirn: u8) -> u8 {
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

// Turns a hall direction const into the corresponding elevator direction const
pub fn halldirn_to_elevdirn(dirn: u8) -> u8 {
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

// Turns a hall direction const into the corresponding const in the opposite direction
pub fn opposite_direction_hall(direction: u8) -> u8 {
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

// Recieves the destination_list and returns the target floor; the last destination in our current direction
pub fn target_floor_function(dirn: u8, destination_list: HashSet<Order>, last_floor: u8) -> Option<u8> {
    // print!("\nTARGET FLOOR\n");
    // print!("dirn  {:#?}\n",dirn);
    // print!("destination_list  {:#?}\n",destination_list);
    // print!("last_floor  {:#?}\n",last_floor);
    
    if destination_list.is_empty() {
        return None;
    }

    let mut destination_list_vec = Vec::new();
    for order in destination_list {
        destination_list_vec.insert(0, order.floor_number);
    }
    match dirn {
        e::DIRN_UP => {
            let target_floor = destination_list_vec.iter().max();
            // println!("TARGET: {:#?}",target_floor.copied());
            return target_floor.copied();
        }
        e::DIRN_DOWN => {
            let target_floor = destination_list_vec.iter().min();
            // println!("TARGET: {:#?}",target_floor.copied());
            return target_floor.copied();
        }
        e::DIRN_STOP => {
            // println!("TARGET: {:#?}",Some(last_floor));
            return Some(last_floor);
        }
        2_u8..=254_u8 => {
            println!("Error getting target floor");
            return Some(last_floor);
        }
    }
}