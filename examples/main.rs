extern crate monkeys;

use monkeys::async;

fn main() {
    loop {
        async(|_| 3).get();
    }
}
