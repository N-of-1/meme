use crate::muse_model::{MuseMessage, MuseMessageType};
/// Muse packets are received over an OSC protol USP socket from MindMonitor app
/// running on Android on the same WIFI
use log::*;
use std::net::SocketAddr;

use chrono::Local;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use nannou_osc::*;

pub fn parse_muse_packet(addr: SocketAddr, packet: &Packet) -> Vec<MuseMessage> {
    let mut raw_messages = Vec::new();
    let message_time = Local::now();

    packet.clone().unfold(&mut raw_messages);
    let mut muse_messages = Vec::with_capacity(raw_messages.len());

    for raw_message in raw_messages {
        if let Some(muse_message_type) = parse_muse_message_type(raw_message) {
            muse_messages.push(MuseMessage {
                message_time,
                ip_address: addr,
                muse_message_type,
            });
        }
    }

    muse_messages
}

pub fn parse_muse_message_type(raw_message: Message) -> Option<MuseMessageType> {
    let service = raw_message.addr.as_ref();
    let args = raw_message
        .clone()
        .args
        .expect("Expected value was not sent by Muse");

    match (match service {
        "/muse/eeg" => {
            let eeg = [
                get_float_from_args(0, &args),
                get_float_from_args(0, &args),
                get_float_from_args(0, &args),
                get_float_from_args(0, &args),
            ];

            Some(MuseMessageType::Eeg { eeg })
        }

        "/muse/acc" => Some(MuseMessageType::Accelerometer {
            x: get_float_from_args(0, &args),
            y: get_float_from_args(1, &args),
            z: get_float_from_args(2, &args),
        }),

        "/muse/gyro" => Some(MuseMessageType::Gyro {
            x: get_float_from_args(0, &args),
            y: get_float_from_args(1, &args),
            z: get_float_from_args(2, &args),
        }),

        "/muse/elements/touching_forehead" => Some(MuseMessageType::TouchingForehead {
            touch: get_int_from_args(0, &args) != 0,
        }),

        "/muse/elements/horseshoe" => Some(MuseMessageType::Horseshoe {
            a: get_float_from_args(0, &args),
            b: get_float_from_args(1, &args),
            c: get_float_from_args(2, &args),
            d: get_float_from_args(3, &args),
        }),

        "/muse/elements/alpha_absolute" => Some(MuseMessageType::Alpha {
            alpha: [
                get_float_from_args(0, &args),
                get_float_from_args(1, &args),
                get_float_from_args(2, &args),
                get_float_from_args(3, &args),
            ],
        }),

        "/muse/elements/beta_absolute" => Some(MuseMessageType::Beta {
            beta: [
                get_float_from_args(0, &args),
                get_float_from_args(1, &args),
                get_float_from_args(2, &args),
                get_float_from_args(3, &args),
            ],
        }),

        "/muse/elements/gamma_absolute" => Some(MuseMessageType::Gamma {
            gamma: [
                get_float_from_args(0, &args),
                get_float_from_args(1, &args),
                get_float_from_args(2, &args),
                get_float_from_args(3, &args),
            ],
        }),

        "/muse/elements/delta_absolute" => Some(MuseMessageType::Delta {
            a: get_float_from_args(0, &args),
            b: get_float_from_args(1, &args),
            c: get_float_from_args(2, &args),
            d: get_float_from_args(3, &args),
        }),

        "/muse/elements/theta_absolute" => Some(MuseMessageType::Theta {
            a: get_float_from_args(0, &args),
            b: get_float_from_args(1, &args),
            c: get_float_from_args(2, &args),
            d: get_float_from_args(3, &args),
        }),

        "/muse/elements/blink" => {
            let blink = get_int_from_args(0, &args);
            info!("Blink: {:#?}", blink);

            Some(MuseMessageType::Blink { blink: blink != 0 })
        }

        "/muse/batt" => Some(MuseMessageType::Batt {
            batt: (get_int_from_args(1, &args) as f32 / get_int_from_args(0, &args) as f32) as i32,
        }),

        "/muse/elements/jaw_clench" => Some(MuseMessageType::JawClench {
            clench: get_int_from_args(0, &args) != 0,
        }),

        _ => {
            error!("Unparsed message type: {:#?} {:#?}", service, raw_message);
            None
        }
    })
    .clone()
    {
        Some(m) => warn!("OSC message: {:?}", m),
        _ => warn!("Unparsed OSC message"),
    }

    match service {
        "/muse/eeg" => {
            let eeg = [
                get_float_from_args(0, &args),
                get_float_from_args(0, &args),
                get_float_from_args(0, &args),
                get_float_from_args(0, &args),
            ];

            Some(MuseMessageType::Eeg { eeg })
        }

        "/muse/acc" => Some(MuseMessageType::Accelerometer {
            x: get_float_from_args(0, &args),
            y: get_float_from_args(1, &args),
            z: get_float_from_args(2, &args),
        }),

        "/muse/gyro" => Some(MuseMessageType::Gyro {
            x: get_float_from_args(0, &args),
            y: get_float_from_args(1, &args),
            z: get_float_from_args(2, &args),
        }),

        "/muse/elements/touching_forehead" => Some(MuseMessageType::TouchingForehead {
            touch: get_int_from_args(0, &args) != 0,
        }),

        "/muse/elements/horseshoe" => Some(MuseMessageType::Horseshoe {
            a: get_float_from_args(0, &args),
            b: get_float_from_args(1, &args),
            c: get_float_from_args(2, &args),
            d: get_float_from_args(3, &args),
        }),

        "/muse/elements/alpha_absolute" => {
            let alpha = [
                get_float_from_args(0, &args),
                get_float_from_args(1, &args),
                get_float_from_args(2, &args),
                get_float_from_args(3, &args),
            ];

            // println!("Raw Alpha: [{:#?}, {:#?}, {:#?}, {:#?}]", a, b, c, d);

            Some(MuseMessageType::Alpha { alpha })
        }

        "/muse/elements/beta_absolute" => {
            let beta = [
                get_float_from_args(0, &args),
                get_float_from_args(1, &args),
                get_float_from_args(2, &args),
                get_float_from_args(3, &args),
            ];
            Some(MuseMessageType::Beta { beta })
        }

        "/muse/elements/gamma_absolute" => {
            let gamma = [
                get_float_from_args(0, &args),
                get_float_from_args(1, &args),
                get_float_from_args(2, &args),
                get_float_from_args(3, &args),
            ];
            Some(MuseMessageType::Gamma { gamma })
        }

        "/muse/elements/delta_absolute" => Some(MuseMessageType::Delta {
            a: get_float_from_args(0, &args),
            b: get_float_from_args(1, &args),
            c: get_float_from_args(2, &args),
            d: get_float_from_args(3, &args),
        }),

        "/muse/elements/theta_absolute" => Some(MuseMessageType::Theta {
            a: get_float_from_args(0, &args),
            b: get_float_from_args(1, &args),
            c: get_float_from_args(2, &args),
            d: get_float_from_args(3, &args),
        }),

        "/muse/elements/blink" => {
            let blink = get_int_from_args(0, &args);
            info!("Blink: {:#?}", blink);

            Some(MuseMessageType::Blink { blink: blink != 0 })
        }

        "/muse/batt" => Some(MuseMessageType::Batt {
            batt: (get_int_from_args(1, &args) as f32 / get_int_from_args(0, &args) as f32) as i32,
        }),

        "/muse/elements/jaw_clench" => Some(MuseMessageType::JawClench {
            clench: get_int_from_args(0, &args) != 0,
        }),

        _ => {
            error!("Unparsed message type: {:#?} {:#?}", service, raw_message);
            None
        }
    }
}

fn get_float_from_args(i: usize, args: &Vec<Type>) -> f32 {
    let f = args.get(i).expect("Float was not provided");

    match f {
        Type::Float(value) => *value,
        _ => panic!("Muse value was not a float"),
    }
}

fn get_int_from_args(i: usize, args: &Vec<Type>) -> i32 {
    let j = args.get(i).expect("Int was not provided");
    match j {
        Type::Int(value) => *value,
        _ => panic!("Muse value was not an int"),
    }
}

#[cfg(test)]
mod tests {
    use crate::muse_packet::*;

    #[test]
    fn test_int_from_args() {
        let i = 32;
        let mut args: Vec<Type> = Vec::new();
        args.push(Type::Int(i));

        assert_eq!(i, get_int_from_args(0, &args));
    }

    #[test]
    fn test_float_from_args() {
        let f = 55.0;
        let mut args: Vec<Type> = Vec::new();
        args.push(Type::Float(f));

        assert_eq!(f, get_float_from_args(0, &args));
    }
}
