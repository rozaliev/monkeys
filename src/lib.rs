#![feature(box_syntax)]
#![feature(fnbox)]
#![feature(recover, std_panic)]
#![feature(reflect_marker)]
#![feature(const_fn)]

extern crate context;

use std::marker::{PhantomData, Reflect};
use std::fmt::{self, Debug};
use std::boxed::FnBox;
use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::{self, AssertRecoverSafe};
use std::any::Any;
use std::collections::VecDeque;

use std::sync::atomic::{AtomicUsize, Ordering};

use context::{Context, Transfer, ContextFn};
use context::stack::{ProtectedFixedSizeStack, Stack};


thread_local!(pub static SCHEDULER: Scheduler = Scheduler::new());

static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);


#[derive(PartialEq, Eq)]
enum CoroutineState {
    BlockedOnCo(CoroutineId),
    ReadyToYield,
    Complete,
    Ready,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct CoroutineId(usize);

pub struct Scheduler {
    work_queue: RefCell<VecDeque<Box<CoroutineHandle>>>,
    blocked: RefCell<HashMap<CoroutineId, Box<CoroutineHandle>>>,
    completed: RefCell<HashMap<CoroutineId, Box<CoroutineHandle>>>,
    ready_to_yield: RefCell<HashMap<CoroutineId, Box<CoroutineHandle>>>,
}

struct Handler<Y, R> {
    flow: Option<Flow<Y, R>>,
}

pub struct Flow<Y, R> {
    coroutine: Option<Coroutine<Y, R>>,
}

// FIXME: add drop to remove from sched and dealloc coroutine
pub struct Stream<Y, R> {
    is_done: bool,
    result_co_id: CoroutineId,
    types: PhantomData<(Y, R)>,
}

struct Coroutine<Y, R> {
    coroutine_id: CoroutineId,
    yield_val: Option<Y>,
    result_val: Option<R>,
    transfer: Option<Transfer>,
    state: CoroutineState,
}

extern "C" fn init_coroutine(mut t: context::Transfer) -> ! {
    let body: Box<FnBox(Transfer)> = from_mut_ptr(t.data);

    body(t);

    unimplemented!()
}

pub fn async<F, Y, R>(mut f: F) -> Stream<Y, R>
    where F: FnOnce(&mut Flow<Y, R>) -> R,
          Y: Reflect + 'static,
          R: Reflect + 'static
{
    let stack = ProtectedFixedSizeStack::default();
    let mut transfer = Transfer::new(Context::new(&stack, init_coroutine), 0);

    let mut body = Some((box move |mut t: Transfer| {
        let _ = stack;

        let mut flow = Flow {
            coroutine: Some(Coroutine {
                coroutine_id: CoroutineId::new(),
                yield_val: None,
                result_val: None,
                transfer: Some(t),
                state: CoroutineState::Ready,
            }),
        };



        flow.resume();

        *flow.result_mut() = Some(f(&mut flow));
        *flow.state_mut() = CoroutineState::Complete;


        flow.resume();

        // dealoc stack

        unreachable!()

    }) as Box<FnBox(Transfer)>);


    let body_ptr = to_mut_ptr(&mut body);
    transfer = transfer.context.resume(body_ptr);

    let mut flow: Flow<Y, R> = from_mut_ptr(transfer.data);
    *flow.transfer_mut() = Some(transfer);

    debug_assert!(body.is_none());

    let stream = Stream {
        is_done: false,
        result_co_id: flow.coroutine_id(),
        types: PhantomData,
    };


    SCHEDULER.with(|sc| sc.add_co(box Handler { flow: Some(flow) }));

    stream
}

trait CoroutineHandle {

    fn state(&self) -> &CoroutineState;

    fn state_mut(&mut self) -> &mut CoroutineState;

    fn is_ready(&self) -> bool;

    fn is_complete(&self) -> bool;

    fn is_ready_to_yield(&self) -> bool;

    fn run(&mut self);

    fn co_id(&self) -> CoroutineId;

    fn take_inner(&mut self) -> Box<Any>;

    fn inner_mut(&mut self) -> &mut Any;

}

impl<Y: Reflect + 'static, R: Reflect + 'static> CoroutineHandle for Handler<Y, R> {
    fn state(&self) -> &CoroutineState {
        self.flow.as_ref().unwrap().state()
    }

    fn state_mut(&mut self) -> &mut CoroutineState {
        self.flow.as_mut().unwrap().state_mut()
    }

    fn is_ready(&self) -> bool {
        CoroutineState::Ready == *self.flow.as_ref().unwrap().state()
    }

    fn run(&mut self) {
        let mut flow = self.flow.take().unwrap();
        flow.resume();
        self.flow = Some(flow);
    }

    fn co_id(&self) -> CoroutineId {
        self.flow.as_ref().unwrap().coroutine_id()
    }

    fn is_complete(&self) -> bool {
        CoroutineState::Complete == *self.flow.as_ref().unwrap().state()
    }

    fn is_ready_to_yield(&self) -> bool {
        CoroutineState::ReadyToYield == *self.flow.as_ref().unwrap().state()
    }

    fn take_inner(&mut self) -> Box<Any> {
        box self.flow.take().unwrap()
    }

    fn inner_mut(&mut self) -> &mut Any {
        self.flow.as_mut().unwrap()
    }
}


impl Scheduler {
    fn new() -> Scheduler {
        Scheduler {
            work_queue: RefCell::new(VecDeque::new()),
            blocked: RefCell::new(HashMap::new()),
            completed: RefCell::new(HashMap::new()),
            ready_to_yield: RefCell::new(HashMap::new()),
        }
    }

    fn schedule_till(&self, wait_id: CoroutineId) {
        if let Some(co) = self.ready_to_yield.borrow_mut().remove(&wait_id) {
            self.work_queue.borrow_mut().push_back(co);
        }


        loop {

            // self.resolve_external_compliter(); // for network
            // self.resolve_timers();
            let co_opt = self.work_queue.borrow_mut().pop_front();
            if let Some(mut co) = co_opt {
                co.run();

                match *co.state() {
                    CoroutineState::BlockedOnCo(block_id) => {
                        self.blocked.borrow_mut().insert(block_id, co);
                    }

                    CoroutineState::Complete => {
                        let id = co.co_id();
                        self.completed.borrow_mut().insert(id, co);

                        if let Some(blocked) = self.blocked.borrow_mut().remove(&id) {
                            self.work_queue.borrow_mut().push_back(blocked);
                        }

                        if id == wait_id {
                            return;
                        }
                    }

                    CoroutineState::ReadyToYield => {
                        let id = co.co_id();
                        self.ready_to_yield.borrow_mut().insert(id, co);

                        if let Some(blocked) = self.blocked.borrow_mut().remove(&id) {
                            self.work_queue.borrow_mut().push_back(blocked);
                        }

                        if id == wait_id {
                            return;
                        }
                    }

                    CoroutineState::Ready => {
                        panic!("coroutine {:?} is ready after run ", co.co_id())
                    }
                }
            }


        }
    }



    fn get_yield_for<Y: Reflect + 'static>(&self, co_id: CoroutineId) -> Option<Y> {
        if let Some(_) = self.completed.borrow_mut().remove(&co_id) {
            return None;
        }

        let mut handle_ref = self.ready_to_yield.borrow_mut();
        let handle = handle_ref.get_mut(&co_id).unwrap();

        let flow: &mut Flow<Y, ()> = handle.inner_mut()
                                           .downcast_mut()
                                           .unwrap();

        let opt = flow.coroutine
                      .as_mut()
                      .unwrap()
                      .yield_val
                      .take();







        opt
    }

    fn add_co(&self, handle: Box<CoroutineHandle>) {
        self.work_queue.borrow_mut().push_back(handle);
    }
}


impl CoroutineId {
    fn new() -> CoroutineId {
        CoroutineId(ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl<Y> Flow<Y, ()> {
    pub fn yield_it(&mut self, stream_val: Y) {
        *self.state_mut() = CoroutineState::ReadyToYield;
        *self.yield_mut() = Some(stream_val);
        self.resume();
    }
}

impl<Y, R> Flow<Y, R> {
    pub fn await<A: Reflect + 'static>(&mut self, stream: Stream<(), A>) -> A {
        *self.state_mut() = CoroutineState::BlockedOnCo(stream.result_co_id);
        self.resume();

        let mut handler = SCHEDULER.with(|sc| {
            sc.completed.borrow_mut().remove(&stream.result_co_id).unwrap()
        });

        let mut inner = handler.take_inner();
        let f: &mut Flow<(), A> = inner.downcast_mut().unwrap();
        f.result_mut().take().unwrap()

    }

    fn resume(&mut self) {
        let mut t = self.transfer_mut().take().unwrap();
        let co = self.coroutine.take().unwrap();

        let mut some_self = Some(Flow { coroutine: Some(co) });
        let self_ptr = to_mut_ptr(&mut some_self);
        t = t.context.resume(self_ptr);
        let mut s: Self = from_mut_ptr(t.data);
        *s.transfer_mut() = Some(t);

        self.coroutine = s.coroutine.take();
    }

    fn result_mut(&mut self) -> &mut Option<R> {
        &mut self.coroutine.as_mut().unwrap().result_val
    }

    fn yield_mut(&mut self) -> &mut Option<Y> {
        &mut self.coroutine.as_mut().unwrap().yield_val
    }

    fn state_mut(&mut self) -> &mut CoroutineState {
        &mut self.coroutine.as_mut().unwrap().state
    }

    fn state(&self) -> &CoroutineState {
        &self.coroutine.as_ref().unwrap().state
    }

    fn transfer_mut(&mut self) -> &mut Option<Transfer> {
        &mut self.coroutine.as_mut().unwrap().transfer
    }

    fn coroutine_id(&self) -> CoroutineId {
        self.coroutine.as_ref().unwrap().coroutine_id
    }
}




#[inline(always)]
fn from_mut_ptr<T>(ptr: usize) -> T {
    unsafe {
        let o = &mut *(ptr as *mut Option<T>);
        o.take().unwrap()
    }
}

fn to_mut_ptr<T>(data: &mut Option<T>) -> usize {
    data as *mut _ as usize
}



impl<R: Reflect + 'static> Stream<(), R> {
    pub fn get(self) -> R {
        let mut handler = SCHEDULER.with(|sc| {
            sc.schedule_till(self.result_co_id);
            sc.completed.borrow_mut().remove(&self.result_co_id).unwrap()
        });

        let mut inner = handler.take_inner();
        let f: &mut Flow<(), R> = inner.downcast_mut().unwrap();
        f.result_mut().take().unwrap()
    }
}


impl<Y: Reflect + 'static> Iterator for Stream<Y, ()> {
    type Item = Y;

    fn next(&mut self) -> Option<Y> {
        if self.is_done {
            return None;
        }

        let opt = SCHEDULER.with(|sc| {
            sc.schedule_till(self.result_co_id);
            sc.get_yield_for(self.result_co_id)
        });

        if opt.is_none() {
            self.is_done = true;
        }

        opt
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::thread;

    #[test]
    fn simple_async() {
        let res = async(|_| 3).get();

        assert_eq!(res, 3);

        SCHEDULER.with(|sc| {
            assert_eq!(sc.work_queue.borrow_mut().len(), 0);
            assert_eq!(sc.ready_to_yield.borrow_mut().len(), 0);
            assert_eq!(sc.completed.borrow_mut().len(), 0);
        })
    }

    #[test]
    fn await() {
        let res = async(|flow| flow.await(async(|_| 3))).get();
        assert_eq!(res, 3);

        SCHEDULER.with(|sc| {
            assert_eq!(sc.work_queue.borrow_mut().len(), 0);
            assert_eq!(sc.ready_to_yield.borrow_mut().len(), 0);
            assert_eq!(sc.completed.borrow_mut().len(), 0);
        })
    }

    #[test]
    fn yield_it() {
        let mut stream = async(|flow| {
            for i in 0..5 {
                flow.yield_it(i)
            }
        });

        for i in 0..5 {
            assert_eq!(Some(i), stream.next());
        }

        assert_eq!(None, stream.next());

        SCHEDULER.with(|sc| {
            assert_eq!(sc.work_queue.borrow_mut().len(), 0);
            assert_eq!(sc.ready_to_yield.borrow_mut().len(), 0);
            assert_eq!(sc.completed.borrow_mut().len(), 0);
        })

    }

    #[test]
    fn yield_it_in_async() {
        let r = async(|_| {
                    let mut stream = async(|flow| {
                        let stream2 = async(|flow| {
                            for i in 0..5 {
                                flow.yield_it(i)
                            }
                        });

                        for v in stream2 {
                            flow.yield_it(v)
                        }
                    });

                    for i in 0..5 {
                        assert_eq!(Some(i), stream.next());
                    }

                    assert_eq!(None, stream.next());
                    333
                })
                    .get();



        assert_eq!(r, 333);

        SCHEDULER.with(|sc| {
            assert_eq!(sc.work_queue.borrow_mut().len(), 0);
            assert_eq!(sc.ready_to_yield.borrow_mut().len(), 0);
            assert_eq!(sc.completed.borrow_mut().len(), 0);
        })

    }
}
