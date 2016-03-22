extern crate monkeys;

use monkeys::async;

fn main() {

    let res = async(|flow| flow.await(async(|_| 3))).get();

}
