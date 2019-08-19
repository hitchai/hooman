#![feature(generators, generator_trait)]

use carrier;
use carrier::osaka::{self, osaka};
use env_logger;
use prost::{Message};
use linux_embedded_hal::I2cdev;
use pwm_pca9685::{Channel, Pca9685};
use std::time::{Duration, Instant};

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/hitch.v1.rs"));
}

fn main() -> Result<(), carrier::Error> {
    if let Err(_) = std::env::var("RUST_LOG") {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::Builder::from_default_env().default_format_timestamp(false).init();


    reset();

    let config = carrier::config::load()?;
    let poll = osaka::Poll::new();
    let mut publisher = carrier::publisher::new(config)
        .route("/v0/shell",                         None, carrier::publisher::shell::main)
        .route("/v0/sft",                           None, carrier::publisher::sft::main)
        .route("/v0/tcp",                           None, carrier::publisher::tcp::main)
        .route("/v2/carrier.sysinfo.v1/sysinfo",    None, carrier::publisher::sysinfo::sysinfo)
        .route("/v2/hitch.v1/hooman",               Some(100), drive_handler_)
        .with_disco("hitch".into(), carrier::BUILD_ID.into())
    .publish(poll);
    publisher.run()
}

fn reset() {
    let dev = I2cdev::new("/dev/i2c-1").unwrap();
    let mut pwm = Pca9685::new(dev, pwm_pca9685::SlaveAddr::default());
    pwm.disable().unwrap();
    pwm.set_prescale(100).unwrap();
    pwm.set_channel_on(Channel::C1,  0).unwrap();
    pwm.set_channel_off(Channel::C1,  0).unwrap();
    pwm.enable().unwrap();
}

fn drive_handler_(
    poll: osaka::Poll,
    headers: carrier::headers::Headers,
    _: &carrier::identity::Identity,
    stream: carrier::endpoint::Stream,
    ) -> Option<osaka::Task<()>> {
    Some(drive_handler(poll, headers, stream))
}


#[osaka]
fn drive_handler(
    _poll: osaka::Poll,
    _headers: carrier::headers::Headers,
    mut stream: carrier::endpoint::Stream,
)
{
    println!("hooman connected");
    let _d = carrier::util::defer(|| {
        reset();
        println!("hooman disconnected")
    });
    stream.send(carrier::headers::Headers::ok().encode());



    let dev = I2cdev::new("/dev/i2c-1").unwrap();
    let mut pwm = Pca9685::new(dev, pwm_pca9685::SlaveAddr::default());
    pwm.set_prescale(100).unwrap();
    pwm.enable().unwrap();



    let mut moving_forward  = false;
    let mut breaking        = false;

    loop {
        let m = osaka::sync!(stream);
        let m = proto::HoomanToHitch::decode(&m).unwrap();
        //println!("{:#?}", m);

        match m.m {
            Some(proto::hooman_to_hitch::M::Target(proto::Moving{mut x,roll})) => {



                if x < 390 && ! breaking {
                    if moving_forward {
                        println!("BREAK");
                        breaking = true;
                    }
                    if x < 372 {
                        x = 372;
                    }
                    moving_forward = false;
                }

                if x >= 390 {
                    breaking = false;
                }

                if x > 390 {
                    moving_forward = true;
                }

                if x > 395 {
                    x = 395;
                }

                pwm.set_channel_on(Channel::C2,  0).unwrap();
                pwm.set_channel_off(Channel::C2,  x as u16).unwrap();

                pwm.set_channel_on(Channel::C1,  0).unwrap();
                pwm.set_channel_off(Channel::C1,  roll as u16).unwrap();

                /*
                if roll == 0 {
                    servos.set_pwm(1, 0, ROLL_CENTER).unwrap();
                } else if roll < 255 && roll > - 255 {
                    let roll = roll as f32 / 255.0;
                    println!("roll: {}", roll);
                    let roll = ROLL_CENTER as f32 + (ROLL_FULL_ANGLE as f32 * roll);
                    println!("    : {}", roll);
                    println!("    : {}", roll as u8);
                    servos.set_pwm(1, 0, roll as u8).unwrap();
                }
                */

                stream.message(proto::HitchToHooman{
                    sync: m.sync,
                    m: Some(proto::hitch_to_hooman::M::Moving(proto::Moving{x,roll})),
                });
            },
            _ => (),
        }
    }
}
