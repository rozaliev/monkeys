extern crate monkeys;

use monkeys::async;

fn main() {

    {
        let mut stream = async(|flow| {
            for i in 0..5 {
                let mut s = async(|flow| flow.yield_it(0));
                let mut s2 = async(|flow| flow.yield_it(1));
                s2.next();
                s2.next();

                flow.yield_it(i);
            }
        });

        let _ = stream.next();
    }
}
