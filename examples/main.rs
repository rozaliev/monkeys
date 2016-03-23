extern crate monkeys;

use monkeys::async;

fn main() {

    let mut stream = async(|flow| {
        for i in 0..5 {
            flow.yield_it(i)
        }
    });

    for i in 0..5 {
        let val = stream.next();
        println!("got: {:?}", val);
        assert_eq!(Some(i), val);
    }

    assert_eq!(stream.next(), None);

}
