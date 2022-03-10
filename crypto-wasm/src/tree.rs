extern crate alloc;

use core::cmp;
use core::iter::Iterator;
use alloc::vec::Vec;

use crate::errors;

#[derive(Debug, Clone)]
struct Node<'a> {
    pub head: &'a Node<'a>,
    pub left: Option<&'a Node<'a>>,
    pub right: Option<&'a Node<'a>>,
    pub height: u8
}

impl<'a> cmp::PartialEq for Node<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.height == other.height
    }
}

impl<'a> cmp::Eq for Node<'a> {}

impl<'a> cmp::Ord for Node<'a> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.height.cmp(&other.height)
    }
}

impl<'a> cmp::PartialOrd for Node<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
struct AVLTree<'a> {
    root: Option<&'a Node<'a>>
}

// Am using a loop vs recursion to prevent stack overflows in the event of very large trees
struct AVLTreeIter<'a> {
    outstanding: Vec<&'a Node<'a>>,
    curr: Option<&'a Node<'a>>
}

impl<'a> AVLTreeIter<'a> {
    fn _match(&mut self, node: &'a Node<'a>) -> Option<&'a Node<'a>> {
        match node.left {
            Some(left) => {
                self.outstanding.push(left);
                self.curr = Some(left);
            },
            None => self.curr = None
        }

        match node.right {
            Some(right) => self.curr = Some(right),
            None => return Some(node)
        }

        return Some(node);
    }
}

impl<'a> Iterator for AVLTreeIter<'a> {
    type Item = &'a Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.curr {
            Some(node) => self._match(node),
            None => match self.outstanding.pop() {
                Some(next) => self._match(next),
                None => None
            }
        }
    }
}

impl<'a> AVLTree<'a> {
    fn new() -> Self {
        Self {
            root: None
        }
    }

    fn iter(&'a self) -> AVLTreeIter<'a> {
        AVLTreeIter {
            outstanding: Vec::new(),
            curr: self.root
        }
    }
}

trait Operations<'a> {
    fn insert(node: &'a Node) -> Result<&'a Node<'a>, errors::AVLError<'a>>;
    fn remove(node: &'a Node) -> Result<&'a Node<'a>, errors::AVLError<'a>>;
    fn rebalance();
}

impl<'a> Operations<'a> for AVLTree<'a> {
    fn insert(node: &'a Node) -> Result<&'a Node<'a>, errors::AVLError<'a>> {
        return Ok(node);
    }

    fn remove(node: &'a Node) -> Result<&'a Node<'a>, errors::AVLError<'a>> {
        return Ok(node);
    }

    fn rebalance() {
        return;
    }
}
