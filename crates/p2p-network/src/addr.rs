use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct NodeId(pub String);

impl From<String> for NodeId {
    fn from(value: String) -> Self {
        NodeId(value)
    }
}

impl Deref for NodeId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
