#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct NodeId(pub String);

impl From<String> for NodeId {
    fn from(value: String) -> Self {
        NodeId(value)
    }
}
