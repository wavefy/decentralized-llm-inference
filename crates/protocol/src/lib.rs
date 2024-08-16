#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Session(u64);
impl Session {
    pub fn new() -> Self {
        Self(rand::random())
    }
}
