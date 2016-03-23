extern crate monkeys;

use monkeys::async;

fn main() {
    let r = async(|_| {
                let mut stream = async(|flow| {
                    let stream2 = async(|flow| {
                        for i in 0..5 {
                            flow.yield_it(i)
                        }
                    });

                    for v in stream2 {
                        flow.yield_it(v + 1)
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


}
