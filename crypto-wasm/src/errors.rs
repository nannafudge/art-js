use core::fmt;

#[derive(Debug, Clone)]
pub struct AVLError;

impl fmt::Display for AVLError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "Invalid AVL Tree Operation!");
    }
}