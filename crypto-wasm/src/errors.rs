extern crate alloc;

use core::fmt;

#[derive(Debug, Clone)]
pub struct AVLError<'a> {
    pub reason: &'a str
}

impl<'a> fmt::Display for AVLError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "Invalid AVL Tree Operation: {}", self.reason);
    }
}

#[derive(Debug, Clone)]
pub struct ECError<'a> {
    pub reason: &'a str
}

impl<'a> fmt::Display for ECError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "Invalid EC Operation: {}", self.reason);
    }
}