/// Forked from https://github.com/Diggsey/lockless under the MIT license
///
/// Copyright 2017 Diggory Blake
///
/// Permission is hereby granted, free of charge, to any person obtaining a
/// copy of this software and associated documentation files (the "Software"),
/// to deal in the Software without restriction, including without limitation
/// the rights to use, copy, modify, merge, publish, distribute, sublicense,
/// and/or sell copies of the Software, and to permit persons to whom the
/// Software is furnished to do so, subject to the following conditions:
///
/// The above copyright notice and this permission notice shall be included
/// in all copies or substantial portions of the Software.
///
/// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
/// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
/// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
/// THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
/// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
/// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
/// IN THE SOFTWARE.

/// AtomicList is a low-level primitive supporting two safe operations:
/// `push`, which prepends a node to the list and into_iter() which consumes and
/// enumerates the receiver.

use std::sync::atomic::{AtomicPtr, Ordering};
use std::{ptr, mem};

pub type NodePtr<T> = Option<Box<Node<T>>>;

#[derive(Debug)]
pub struct Node<T> {
    pub value: T,
    pub next: NodePtr<T>
}

#[derive(Debug)]
pub struct AtomicList<T>(AtomicPtr<Node<T>>);

fn replace_forget<T>(dest: &mut T, value: T) {
    mem::forget(mem::replace(dest, value))
}

fn into_raw<T>(ptr: NodePtr<T>) -> *mut Node<T> {
    match ptr {
        Some(b) => Box::into_raw(b),
        None => ptr::null_mut()
    }
}

unsafe fn from_raw<T>(ptr: *mut Node<T>) -> NodePtr<T> {
    if ptr == ptr::null_mut() {
        None
    } else {
        Some(Box::from_raw(ptr))
    }
}

impl<T> AtomicList<T> {
    pub fn new() -> Self {
        AtomicList(AtomicPtr::new(into_raw(None)))
    }

    pub fn push(&self, value: T) {
        let mut node = Box::new(Node { value: value, next: None });

        let mut current = self.0.load(Ordering::Relaxed);
        loop {
            replace_forget(&mut node.next, unsafe { from_raw(current) });
            match self.0.compare_exchange_weak(current, &mut *node, Ordering::AcqRel, Ordering::Relaxed) {
                Ok(_) => {
                    mem::forget(node);
                    return
                },
                Err(p) => current = p
            }
        }
    }
}

impl<T> Drop for AtomicList<T> {
    fn drop(&mut self) {
        let p = self.0.swap(into_raw(None), Ordering::Relaxed);
        unsafe { from_raw(p) };
    }
}

impl<T> IntoIterator for AtomicList<T> {
    type Item = T;
    type IntoIter = AtomicListIterator<T>;

    fn into_iter(self) -> Self::IntoIter {
        let raw = self.0.swap(into_raw(None), Ordering::Relaxed);
        AtomicListIterator(AtomicPtr::new(raw))
    }
}

#[derive(Debug)]
pub struct AtomicListIterator<T>(AtomicPtr<Node<T>>);

impl<T> Iterator for AtomicListIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let p = self.0.load(Ordering::Acquire);
        unsafe { from_raw(p) }
            .map(|node| {
                let node = *node;
                let Node { value, next } = node;
                self.0.store(into_raw(next), Ordering::Release);
                value
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push() {
        let list = AtomicList::new();
        list.push(1);
        list.push(2);
        list.push(3);
    }

    #[test]
    fn test_into_iter() {
        let list = AtomicList::new();
        list.push(1);
        list.push(2);
        list.push(3);

        let list: Vec<_> = list.into_iter().collect();
        assert_eq!(list, vec![3, 2, 1]);
    }
}
