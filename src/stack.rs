use std::ops::Deref;
use std::cell::RefCell;

use context::stack::{ProtectedFixedSizeStack, Stack};


thread_local!(static STACK_POOL: StackPool = StackPool::new());

pub struct PooledStack(Option<ProtectedFixedSizeStack>);

struct StackPool {
    pool: RefCell<Vec<ProtectedFixedSizeStack>>,
}

impl StackPool {
    fn new() -> StackPool {
        let mut v = Vec::with_capacity(500);

        for _ in 0..500 {
            v.push(ProtectedFixedSizeStack::default());
        }

        StackPool { pool: RefCell::new(v) }
    }

    fn give(&self, stack: ProtectedFixedSizeStack) {
        self.pool.borrow_mut().push(stack);

    }

    fn get(&self) -> ProtectedFixedSizeStack {
        let mut stack_opt = self.pool.borrow_mut().pop();
        if stack_opt.is_none() {
            self.grow();
            stack_opt = self.pool.borrow_mut().pop();
        }

        stack_opt.unwrap()
    }

    fn grow(&self) {
        let mut p = self.pool.borrow_mut();

        for _ in 0..p.capacity() * 2 {
            p.push(ProtectedFixedSizeStack::default())
        }
    }
}


impl PooledStack {
    pub fn new() -> PooledStack {
        STACK_POOL.with(|p| PooledStack(Some(p.get())))

    }
}


impl Drop for PooledStack {
    fn drop(&mut self) {
        let stack = self.0.take().unwrap();
        STACK_POOL.with(|p| p.give(stack))
    }
}


impl Deref for PooledStack {
    type Target = Stack;

    fn deref(&self) -> &Stack {
        &*self.0.as_ref().unwrap()
    }
}
