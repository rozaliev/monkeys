extern crate monkeys;

use monkeys::async;

fn main() {
    loop {
        async(|f| {
            f.await(async(|_| 3));
        })
            .get();
    }
}
