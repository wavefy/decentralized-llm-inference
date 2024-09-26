pub mod shared_map;

/// generate a random node id with 8 characters
pub fn random_node_id() -> String {
    let random_number = rand::random::<u32>();
    format!("{:08x}", random_number)
}
