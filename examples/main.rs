extern crate monkeys;

use monkeys::async;

fn main() {

    let mut stream = async(|flow| {
        for i in 0..5 {
            flow.yield_it(i)
        }
    });

    for i in 0..5 {
        let v = stream.next();
        println!("v: {:?}", v);
        assert_eq!(Some(i), v);
    }

    assert_eq!(None, stream.next());

}
