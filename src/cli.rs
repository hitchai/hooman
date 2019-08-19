#![feature(generators, generator_trait)]

use carrier;
use carrier::osaka::{self, osaka};
use env_logger;
use prost::{Message};
use stick::Port;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

fn spawn_gamepad(roll: Arc<AtomicI32>, motor: Arc<AtomicI32>) {
    std::thread::spawn(move ||{
        // Connect to all devices.
        let mut port = Port::new();

        // Loop showing state of all devices.
        loop {
            // Cycle through all currently plugged in devices.
            let id = if let Some(a) = port.poll() {
                a
            } else {
                continue;
            };

            if let Some(state) = port.get(id) {
                let (x,y) = state.cam().expect("cam stick");
                let x = (255.0 * x) as i32;
                let y = (255.0 * y) as i32;

                println!("{}|{}", x, y);

                roll.store(x, Ordering::SeqCst);
                motor.store(y, Ordering::SeqCst);
            }
        }
    });
}


pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/hitch.v1.rs"));
}

fn main() -> Result<(), carrier::Error> {
    if let Err(_) = std::env::var("RUST_LOG") {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::Builder::from_default_env().default_format_timestamp(false).init();

    let config = carrier::config::load()?;

    let target = config
        .resolve_identity(std::env::args().nth(1).unwrap().to_string())
        .expect("resolving identity from cli");

    let headers = carrier::headers::Headers::with_path("/v2/hitch.v1/hooman");

    carrier::connect(config).open(target, headers, drive_handler).run()
}


#[osaka]
fn drive_handler(poll: osaka::Poll, _ep: carrier::endpoint::Handle, mut stream: carrier::endpoint::Stream) {
    use osaka::Future;

    let roll  = Arc::new(AtomicI32::new(0));
    let motor = Arc::new(AtomicI32::new(0));
    spawn_gamepad(roll.clone(), motor.clone());

    let _d = carrier::util::defer(|| {
        std::process::exit(0);
    });
    let headers = carrier::headers::Headers::decode(&osaka::sync!(stream)).unwrap();
    println!("{:?}", headers);
    let mut now = Instant::now();

    loop {
        if now.elapsed().as_millis() >= 100 {
            now = Instant::now();
            let roll = roll.load(Ordering::SeqCst);
            let roll = roll as f32 / 255.0;
            println!("roll: {}", roll);
            let roll = 390.0 + (50.0 * roll);
            println!("    : {}", roll);
            let roll = roll as u16;
            println!("    : {}", roll);

            let motor = motor.load(Ordering::SeqCst);
            let motor = motor as f32 / 255.0;
            println!("motor: {}", motor);
            let motor = 390.0 + (50.0 * motor);
            println!("    : {}", motor);
            let mut motor = motor as u16;
            println!("    : {}", motor);


            stream.message(proto::HoomanToHitch{
                sync: 1,
                m: Some(proto::hooman_to_hitch::M::Target(proto::Moving{
                    x: motor as i32,
                    roll: roll as i32,
                })),
            });
        }


        match stream.poll() {
            osaka::FutureResult::Again(mut a) => {
                a.merge(poll.later(std::time::Duration::from_millis(10)));
                yield a;
            },
            osaka::FutureResult::Done(m) => {
                let m = proto::HitchToHooman::decode(&m).unwrap();

                //println!("{:#?}", m);

                match m.m {
                    Some(proto::hitch_to_hooman::M::Moving(proto::Moving{x,roll})) => {
                        stream.message(proto::HoomanToHitch{
                            sync: m.sync,
                            m: None,
                        });
                    },
                    _ => (),
                }
            }
        }
    }
}

