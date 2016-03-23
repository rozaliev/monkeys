#![feature(test)]

extern crate test;
extern crate monkeys;

use test::black_box;

use test::Bencher;

use monkeys::async;


#[bench]
fn bench_simple(b: &mut Bencher) {
    b.iter(|| {
        async(|_| 3).get();
    })
}


#[bench]
fn bench_simple_ref(b: &mut Bencher) {
    let f = || 3;

    b.iter(|| {
        black_box(f());
    })
}
