# Monkeys are new coroutines

This is pre-alpha library, nothing works as expected yet.


## Goals

Coroutines with async/await/yield for Rust.


```Rust
fn main() {

   async(|flow| {

     let stream = async(|flow| {
        for i in 0..5 {
            flow.yield_it(i);
        }
      });

      for r in stream {
        println!("got streamed: {:?}", r);
      }


      let awaitable = async(|flow | {
        1 + 1
      });

      let r = flow.await(awaitable);

      println!("awaitable res: {:?}", r);


   }).get();
}

```
prints
``` Rust
> got streamed: 0
> got streamed: 1
> got streamed: 2
> got streamed: 3
> got streamed: 4
> awaitable res: 2
```

## Non goals

There won't be network stack itself. Monkeys are building blocks for higher level libraries.


## Plan

* ~~Initial prototype~~
* Simple timers
* Support for pluggable network stack
* Benchmarking and optimizations