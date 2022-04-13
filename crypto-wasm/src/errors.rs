extern crate alloc;

use core::fmt;

#[derive(Debug, Clone)]
pub struct RatchetError<'a> {
    pub reason: &'a str,
    pub index: usize,
    pub height: usize
}

impl<'a> fmt::Display for RatchetError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "Invalid Ratchet Tree Operation: {:?}, height: {:#}, index: {:#}", self.reason, self.height, self.index);
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