use crate::muse_packet::*;

//#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]

/// Muse data model and associated message handling from muse_packet
// #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use std::sync::mpsc::SendError;

// use log::*;
use chrono::{DateTime, Local};
use csv::Writer;
use num_traits::float::Float;
use std::f32::consts::E;
use std::net::SocketAddr;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::{convert::From, fs::File, slice::Iter, thread};

const FOREHEAD_COUNTDOWN: i32 = 5; // 60th of a second counts
const BLINK_COUNTDOWN: i32 = 5;
const CLENCH_COUNTDOWN: i32 = 5;
const HISTORY_LENGTH: usize = 120; // Used to trunacte ArousalHistory and ValenceHistory length - this is the number of samples in the normalization phase
const TP9: usize = 0; // Muse measurment array index for first electrode
const AF7: usize = 1; // Muse measurment array index for second electrode
const AF8: usize = 2; // Muse measurment array index for third electrode
const TP10: usize = 3; // Muse measurment array index for fourth electrode

const WINDOW_LENGTH: usize = 10; // Current values is smoothed by most recent X values

const OSC_PORT: u16 = 34254;

const TIME_FORMAT_FOR_FILENAMES: &str = "%Y-%m-%d %H-%M-%S%.3f"; // 2020-02-25 09-35-49
const TIME_FORMAT_FOR_CSV: &str = "%Y-%m-%d %H:%M:%S%.3f"; // 2020-02-25 09:35:49

/// Make it easier to print out the message receiver object for debug purposes
// struct ReceiverDebug<T> {
//     receiver: osc::Receiver<T>,
// }

// impl Debug for ReceiverDebug<T> {
//     fn fmt(&self, f: &mut Formatter<T>) -> fmt::Result {
//         write!(f, "<Receiver>")
//     }
// }

/// Format the current instant of time nominally accurate down to milliseconds and without any : to make it compatible with all file system file names
pub fn date_time_filename_format(date_time: DateTime<Local>) -> String {
    let s: String = format!("{}", date_time.format(TIME_FORMAT_FOR_FILENAMES));

    s
}

/// Format a Duration (from packet receive time etc) to a string date nominally accurate down to milliseconds in a format sutable for parsing from CSV / Spreadsheets
fn date_time_csv_format(date_time: DateTime<Local>) -> String {
    let s: String = format!("{}", date_time.format(TIME_FORMAT_FOR_CSV));

    s
}

// fn current_date_time_csv_format() -> String {
//     let date = Local::now();
//     let s: String = format!("{}", date.format(TIME_FORMAT_FOR_CSV));

//     s
// }

/// The different display modes supported for live screen updates based on Muse EEG signals
#[derive(Clone, Debug)]
pub enum DisplayType {
    Mandala,
    Dowsiness,
    Emotion,
    EegValues,
}

#[derive(Clone, Debug)]
pub enum MuseMessageType {
    Eeg { eeg: [f32; 4] }, // microVolts
    Accelerometer { x: f32, y: f32, z: f32 },
    Gyro { x: f32, y: f32, z: f32 },
    Alpha { alpha: [f32; 4] },                // microVolts
    Beta { beta: [f32; 4] },                  // microVolts
    Gamma { gamma: [f32; 4] },                // microVolts
    Delta { a: f32, b: f32, c: f32, d: f32 }, // microVolts
    Theta { a: f32, b: f32, c: f32, d: f32 }, // microVolts
    Batt { batt: i32 },
    Horseshoe { a: f32, b: f32, c: f32, d: f32 },
    TouchingForehead { touch: bool },
    Blink { blink: bool },
    JawClench { clench: bool },
}

type TimedMuseMessage = (DateTime<Local>, MuseMessageType);

#[derive(Clone, Debug)]
pub struct MuseMessage {
    pub message_time: DateTime<Local>, // Since UNIX_EPOCH, the beginning of 1970
    pub ip_address: SocketAddr,
    pub muse_message_type: MuseMessageType,
}

/// Receive messages of EEG data from some source (OSC or websockets)
trait EegMessageReceiver {
    fn new() -> inner_receiver::InnerMessageReceiver;
    fn receive_packets(&self) -> Vec<MuseMessage>;
}

/// An OSC USB packet receiver for all platforms except WASM
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod inner_receiver {
    use super::{EegMessageReceiver, MuseMessage};
    use nannou_osc;

    pub struct InnerMessageReceiver {
        receiver: nannou_osc::Receiver,
    }

    impl EegMessageReceiver for InnerMessageReceiver {
        fn new() -> InnerMessageReceiver {
            info!("Connecting to EEG");

            let receiver = nannou_osc::receiver(super::OSC_PORT)
                .expect("Can not bind to port- is another copy of this app already running?");

            InnerMessageReceiver { receiver }
        }

        /// Receive any pending osc packets.
        fn receive_packets(&self) -> Vec<MuseMessage> {
            let receivables: Vec<(nannou_osc::Packet, std::net::SocketAddr)> =
                self.receiver.try_iter().collect();

            let mut muse_messages: Vec<MuseMessage> = Vec::new();

            for (packet, addr) in receivables {
                let mut additional_messages: Vec<MuseMessage> =
                    super::parse_muse_packet(addr, &packet);
                muse_messages.append(&mut additional_messages);
            }

            muse_messages
        }
    }
}

/// A placeholder structure for WASM to avoid dependency on non-existing package issues
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod inner_receiver {
    use super::{EegMessageReceiver, MuseMessage};

    /// TODO Receive messages from the server in the web implementation
    pub struct InnerMessageReceiver {}

    impl EegMessageReceiver for InnerMessageReceiver {
        fn new() -> InnerMessageReceiver {
            info!("PLACEHOLDER: Will be indirectly connecting to EEG");

            InnerMessageReceiver {}
        }

        /// Receive any pending osc packets.
        fn receive_packets(&self) -> Vec<MuseMessage> {
            Vec::new()
        }
    }
}

pub struct NormalizedValue<T: Float + From<i16>> {
    current: Option<T>,
    min: Option<T>,
    max: Option<T>,
    mean: Option<T>,
    deviation: Option<T>,
    history: Vec<T>,
    moving_average_history: Vec<T>,
}

impl<T> NormalizedValue<T>
where
    T: Float + From<i16>,
{
    pub fn new() -> Self {
        Self {
            current: None,
            min: None,
            max: None,
            mean: None,
            deviation: None,
            history: Vec::new(),
            moving_average_history: Vec::new(),
        }
    }

    pub fn moving_average(&self) -> Option<T>
    where
        T: Float + From<i16>,
    {
        let count = self.moving_average_history.len() as i16;
        let sum = sum(&self.moving_average_history);

        match count {
            positive if positive > 0 => Some(sum / count.into()),
            _ => None,
        }
    }

    // Set the value if it is a change and a rational number. Returns true if the value is accepted as finite and a change from the previous value
    pub fn set(&mut self, val: T) -> bool {
        let acceptable_new_value = match self.current {
            Some(current_value) => val.is_finite() && val != current_value,
            None => val.is_finite(),
        };

        if acceptable_new_value {
            self.current = Some(val);
            if !self.max.is_some() || self.max.unwrap() < val {
                self.max = Some(val);
            }
            if !self.min.is_some() || self.min.unwrap() > val {
                self.min = Some(val);
            }
            self.history.push(val);
            if self.history.len() > HISTORY_LENGTH {
                self.history.remove(0);
            }
            self.mean = mean(&self.history); //TODO never call this anywhere else
            self.deviation = std_deviation(&self.history, self.mean); //TODO never call this anywhere else
            self.moving_average_history.push(val);
            if self.moving_average_history.len() >= WINDOW_LENGTH {
                self.moving_average_history.remove(0);
            }
        }

        acceptable_new_value
    }

    pub fn _percent_normalization_complete(&self) -> f32 {
        self.history.len() as f32 / HISTORY_LENGTH as f32
    }

    pub fn mean(&self) -> Option<T> {
        self.mean
    }

    pub fn deviation(&self) -> Option<T> {
        self.deviation
    }

    pub fn _percent(&self) -> Option<T> {
        match self.current {
            Some(v) => {
                let v100: T = (v - self.min.unwrap()) * 100.into();
                let range: T = self.max.unwrap() - self.min.unwrap();
                let r = v100 / range;

                match r.is_finite() {
                    true => Some(r),
                    false => Some(0.into()),
                }
            }
            None => None,
        }
    }

    // Return the current value normalized based on the initial calibration period
    pub fn normalize(&self, val: Option<T>) -> Option<T> {
        match val {
            Some(v) => {
                let mean_and_deviation = (self.mean(), self.deviation());

                match mean_and_deviation {
                    (Some(mean), Some(deviation)) => Some((v - mean) / deviation),
                    _ => None,
                }
            }
            None => None,
        }
    }
}

fn sum<T>(data: &Vec<T>) -> T
where
    T: Float + From<i16>,
{
    let mut sum: T = 0.into();

    for t in data {
        sum = sum + *t;
    }

    sum
}

fn mean<T>(data: &Vec<T>) -> Option<T>
where
    T: Float + From<i16>,
{
    let count = data.len() as i16;
    let sum = sum(data);

    match count {
        positive if positive > 0 => Some(sum / count.into()),
        _ => None,
    }
}

/// Average the raw values
pub fn average_from_front_electrodes(x: &[f32; 4]) -> f32 {
    (E.powf(x[1]) + E.powf(x[2])) / 2.0
    //(x[0] + x[1] + x[2] + x[3]) / 4.0
    //(x[1] + x[2]) / 2.0
}

/// Create a log of values and events collected during a session
fn create_log_writer(start_date_time: DateTime<Local>, filename: &str) -> Writer<File> {
    let formatted_date_time = date_time_filename_format(start_date_time);

    let filename_with_date_time = format!("{} {}", formatted_date_time, filename);
    let writer: Writer<File> =
        Writer::from_path(filename_with_date_time).expect("Could not open CSV file for writing");

    writer
}

fn write_record(
    time: DateTime<Local>,
    iter: Iter<f32>,
    writer: &mut Writer<File>,
) -> Result<(), String> {
    let mut vec: Vec<String> = Vec::new();
    vec.push(date_time_csv_format(time));
    for val in iter {
        vec.push(val.to_string());
    }
    writer
        .write_record(vec)
        .or(Err("Can not write record".to_string()))
}

fn create_async_eeg_log_writer(
    start_date_time: DateTime<Local>,
    filename: &str,
    header: Iter<&str>,
) -> Sender<MuseMessage> {
    let (tx_log, rx_log): (Sender<MuseMessage>, Receiver<MuseMessage>) = mpsc::channel();
    let filename: String = filename.into();
    let mut header_vec: Vec<String> = Vec::new();
    for val in header {
        header_vec.push(val.to_string());
    }

    thread::spawn(move || {
        let mut writer = create_log_writer(start_date_time, &filename);
        let mut iter: mpsc::Iter<MuseMessage> = rx_log.iter();
        let mut stream_open = true;

        writer
            .write_record(header_vec)
            .expect("Could not write eeg header");
        while stream_open {
            match iter.next() {
                Some(MuseMessage {
                    message_time,
                    muse_message_type,
                    ..
                }) => match muse_message_type {
                    MuseMessageType::Eeg { eeg } => {
                        write_record(message_time, eeg.iter(), &mut writer)
                            .expect(&format!("Could not write record to {}", filename));
                    }
                    _ => {
                        panic!(format!(
                            "Unexpected message type, should be for {}",
                            filename
                        ));
                    }
                },
                None => {
                    writer
                        .flush()
                        .expect(&format!("Can not flush writer: {}", filename));
                    stream_open = false;
                }
            }
        }
    });

    tx_log
}

fn create_async_alpha_log_writer(
    start_date_time: DateTime<Local>,
    filename: &str,
    header: Iter<&str>,
) -> Sender<MuseMessage> {
    let (tx_log, rx_log): (Sender<MuseMessage>, Receiver<MuseMessage>) = mpsc::channel();
    let filename: String = filename.into();
    let mut header_vec: Vec<String> = Vec::new();
    for val in header {
        header_vec.push(val.to_string());
    }

    thread::spawn(move || {
        let mut writer = create_log_writer(start_date_time, &filename);
        let mut iter: mpsc::Iter<MuseMessage> = rx_log.iter();
        let mut stream_open = true;

        writer
            .write_record(header_vec)
            .expect("Could not write alpha header");
        while stream_open {
            match iter.next() {
                Some(MuseMessage {
                    message_time,
                    muse_message_type,
                    ..
                }) => match muse_message_type {
                    MuseMessageType::Alpha { alpha } => {
                        write_record(message_time, alpha.iter(), &mut writer)
                            .expect(&format!("Could not write record to {}", filename));
                    }
                    _ => {
                        panic!(format!(
                            "Unexpected message type, should be for {}",
                            filename
                        ));
                    }
                },
                None => {
                    writer
                        .flush()
                        .expect(&format!("Can not flush writer: {}", filename));
                    stream_open = false;
                }
            }
        }
    });

    tx_log
}

fn create_async_beta_log_writer(
    start_date_time: DateTime<Local>,
    filename: &str,
    header: Iter<&str>,
) -> Sender<MuseMessage> {
    let (tx_log, rx_log): (Sender<MuseMessage>, Receiver<MuseMessage>) = mpsc::channel();
    let filename: String = filename.into();
    let mut header_vec: Vec<String> = Vec::new();
    for val in header {
        header_vec.push(val.to_string());
    }

    thread::spawn(move || {
        let mut writer = create_log_writer(start_date_time, &filename);
        let mut iter: mpsc::Iter<MuseMessage> = rx_log.iter();
        let mut stream_open = true;

        writer
            .write_record(header_vec)
            .expect("Could not write beta header");
        while stream_open {
            match iter.next() {
                Some(MuseMessage {
                    message_time,
                    muse_message_type,
                    ..
                }) => match muse_message_type {
                    MuseMessageType::Beta { beta } => {
                        write_record(message_time, beta.iter(), &mut writer)
                            .expect(&format!("Could not write record to {}", filename));
                    }
                    _ => {
                        panic!(format!(
                            "Unexpected message type, should be for {}",
                            filename
                        ));
                    }
                },
                None => {
                    writer
                        .flush()
                        .expect(&format!("Can not flush writer: {}", filename));
                    stream_open = false;
                }
            }
        }
    });

    tx_log
}

fn create_async_gamma_log_writer(
    start_date_time: DateTime<Local>,
    filename: &str,
    header: Iter<&str>,
) -> Sender<MuseMessage> {
    let (tx_log, rx_log): (Sender<MuseMessage>, Receiver<MuseMessage>) = mpsc::channel();
    let filename: String = filename.into();
    let mut header_vec: Vec<String> = Vec::new();
    for val in header {
        header_vec.push(val.to_string());
    }

    thread::spawn(move || {
        let mut writer = create_log_writer(start_date_time, &filename);
        let mut iter: mpsc::Iter<MuseMessage> = rx_log.iter();
        let mut stream_open = true;

        writer
            .write_record(header_vec)
            .expect("Could not write gamma header");
        while stream_open {
            match iter.next() {
                Some(MuseMessage {
                    message_time,
                    muse_message_type,
                    ..
                }) => match muse_message_type {
                    MuseMessageType::Gamma { gamma } => {
                        write_record(message_time, gamma.iter(), &mut writer)
                            .expect(&format!("Could not write record to {}", filename));
                    }
                    _ => {
                        panic!(format!(
                            "Unexpected message type, should be for {}",
                            filename
                        ));
                    }
                },
                None => {
                    writer
                        .flush()
                        .expect(&format!("Can not flush writer: {}", filename));
                    stream_open = false;
                }
            }
        }
    });

    tx_log
}

/// Snapshot of the most recently collected values from Muse EEG headset
pub struct MuseModel {
    most_recent_message_receive_time: DateTime<Local>,
    pub inner_receiver: inner_receiver::InnerMessageReceiver,
    accelerometer: [f32; 3],
    gyro: [f32; 3],
    pub alpha: [f32; 4],
    pub beta: [f32; 4],
    pub gamma: [f32; 4],
    pub delta: [f32; 4],
    pub theta: [f32; 4],
    batt: i32,
    horseshoe: [f32; 4],
    blink_countdown: i32,
    touching_forehead_countdown: i32,
    jaw_clench_countdown: i32,
    pub scale: f32,
    pub display_type: DisplayType,
    pub arousal: NormalizedValue<f32>,
    pub valence: NormalizedValue<f32>,
    eeg_log_sender: Sender<MuseMessage>, // Raw EEG values every time they arrive, CSV
    alpha_log_sender: Sender<MuseMessage>, // Processed EEG values every time they arrive, CSV
    beta_log_sender: Sender<MuseMessage>, // Processed EEG values every time they arrive, CSV
    gamma_log_sender: Sender<MuseMessage>, // Processed EEG values every time they arrive, CSV
    delta_log_writer: Writer<File>,      // Processed EEG values every time they arrive, CSV
    theta_log_writer: Writer<File>,      // Processed EEG values every time they arrive, CSV
    other_log_writer: Writer<File>,      // Other values every time they arrive, CSV
}

fn std_deviation<T>(data: &Vec<T>, mean: Option<T>) -> Option<T>
where
    T: Float + From<i16>,
{
    match (mean, data.len()) {
        (Some(data_mean), count) if count > 0 => {
            let squared_difference_vec: Vec<T> = data
                .iter()
                .map(|value| {
                    let diff = data_mean - (*value as T);

                    diff * diff
                })
                .collect();

            let variance_sum = sum(&squared_difference_vec) / (count as i16).into();

            Some(variance_sum.sqrt())
        }
        _ => None,
    }
}

impl MuseModel {
    /// Create a new model for storing received values
    pub fn new(start_time: DateTime<Local>) -> MuseModel {
        let inner_receiver = inner_receiver::InnerMessageReceiver::new();
        let eeg_log_sender = create_async_eeg_log_writer(
            start_time,
            "eeg.csv",
            ["Time", "TP9", "AF7", "AF8", "TP10"].iter(),
        );
        let alpha_log_sender = create_async_alpha_log_writer(
            start_time,
            "alpha.csv",
            ["Time", "Alpha TP9", "Alpha AF7", "Alpha AF8", "Alpha TP10"].iter(),
        );
        let beta_log_sender = create_async_beta_log_writer(
            start_time,
            "beta.csv",
            ["Time", "Beta TP9", "Beta AF7", "Beta AF8", "Beta TP10"].iter(),
        );
        let gamma_log_sender = create_async_gamma_log_writer(
            start_time,
            "gamma.csv",
            ["Time", "Gamma TP9", "Gamma AF7", "Gamma AF8", "Gamma TP10"].iter(),
        );
        let mut delta_log_writer = create_log_writer(start_time, "delta.csv");
        delta_log_writer
            .write_record(&["Time", "Delta TP9", "Delta AF7", "Delta AF8", "Delta TP10"])
            .expect("Can not write delta.csv header");
        let mut theta_log_writer = create_log_writer(start_time, "theta.csv");
        theta_log_writer
            .write_record(&["Time", "Theta TP9", "Theta AF7", "Theta AF8", "Theta TP10"])
            .expect("Can not write theta.csv header");
        let mut other_log_writer = create_log_writer(start_time, "other.csv");
        other_log_writer
            .write_record(&["Time", "Record"])
            .expect("Can not write other.csv header");

        MuseModel {
            most_recent_message_receive_time: start_time,
            inner_receiver,
            accelerometer: [0.0, 0.0, 0.0],
            gyro: [0.0, 0.0, 0.0],
            alpha: [0.0, 0.0, 0.0, 0.0], // 7.5-13Hz
            beta: [0.0, 0.0, 0.0, 0.0],  // 13-30Hz
            gamma: [0.0, 0.0, 0.0, 0.0], // 30-44Hz
            delta: [0.0, 0.0, 0.0, 0.0], // 1-4Hz
            theta: [0.0, 0.0, 0.0, 0.0], // 4-8Hz
            batt: 0,
            horseshoe: [0.0, 0.0, 0.0, 0.0],
            blink_countdown: 0,
            touching_forehead_countdown: 0,
            jaw_clench_countdown: 0,
            scale: 1.5, // Make the circles relatively larger or smaller
            display_type: DisplayType::Mandala, // Current drawing mode
            arousal: NormalizedValue::new(),
            valence: NormalizedValue::new(),
            eeg_log_sender,
            alpha_log_sender,
            beta_log_sender,
            gamma_log_sender,
            delta_log_writer,
            theta_log_writer,
            other_log_writer,
        }
    }

    /// Write any pending activity to disk
    pub fn flush_all(&mut self) -> Result<(), std::io::Error> {
        self.theta_log_writer
            .flush()
            .and(self.delta_log_writer.flush())
            .and(self.other_log_writer.flush())
    }

    fn log_delta(&mut self, receive_time: DateTime<Local>) {
        let receive_time_csv_format = date_time_csv_format(receive_time);
        let time = format!("{}", receive_time_csv_format);
        let tp9 = format!("{:?}", self.delta[TP9]);
        let af7 = format!("{:?}", self.delta[AF7]);
        let af8 = format!("{:?}", self.delta[AF8]);
        let tp10 = format!("{:?}", self.delta[TP10]);

        self.delta_log_writer
            .write_record(&[&time, &tp9, &af7, &af8, &tp10])
            .expect("Can not add row to delta.csv");
    }

    fn log_theta(&mut self, receive_time: DateTime<Local>) {
        let receive_time_csv_format = date_time_csv_format(receive_time);
        let time = format!("{}", receive_time_csv_format);
        let tp9 = format!("{:?}", self.theta[TP9]);
        let af7 = format!("{:?}", self.theta[AF7]);
        let af8 = format!("{:?}", self.theta[AF8]);
        let tp10 = format!("{:?}", self.theta[TP10]);

        self.theta_log_writer
            .write_record(&[&time, &tp9, &af7, &af8, &tp10])
            .expect("Can not add row to theta.csv");
    }

    pub fn log_other(&mut self, receive_time: DateTime<Local>, other: &str) {
        let receive_time_csv_format = date_time_csv_format(receive_time);
        let time = format!("{}", receive_time_csv_format);

        self.other_log_writer
            .write_record(&[&time, other])
            .expect("Can not add row to other.csv");
    }

    // pub fn log_other_now(&mut self, other: &str) {
    //     let duration = SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .expect("System clock is not set correctly");
    //     self.log_other(duration, other);
    // }

    /// User has recently clamped their teeth, creating myoelectric interference so interrupting the EEG signal
    pub fn is_jaw_clench(&self) -> bool {
        self.jaw_clench_countdown > 0
    }

    /// User has recently blinked their eyes, creating myoelectric interference so interrupting the EEG signal
    pub fn is_blink(&self) -> bool {
        self.blink_countdown > 0
    }

    /// The Muse headband is recently positioned to touch the user's forehead
    pub fn is_touching_forehead(&self) -> bool {
        self.touching_forehead_countdown > 0
    }

    /// This is called 60x/sec and allows various temporary display states to time out
    pub fn count_down(&mut self) {
        if self.blink_countdown > 0 {
            self.blink_countdown = self.blink_countdown - 1;
        }

        if self.jaw_clench_countdown > 0 {
            self.jaw_clench_countdown = self.jaw_clench_countdown - 1;
        }

        if self.touching_forehead_countdown > 0 {
            self.touching_forehead_countdown = self.touching_forehead_countdown - 1;
        }
    }

    pub fn receive_packets(&mut self) -> (Option<f32>, Option<f32>) {
        let muse_messages = self.inner_receiver.receive_packets();
        let mut updated_numeric_values = false;
        let mut normalized_valence_option = None;
        let mut normalized_arousal_option = None;

        for muse_message in muse_messages {
            self.most_recent_message_receive_time = muse_message.message_time.clone();
            updated_numeric_values = updated_numeric_values
                || self
                    .handle_muse_message(muse_message)
                    .expect("Could not receive OSC message");
        }

        if updated_numeric_values {
            let _valence_updated = self.update_valence();
            let _arousal_updated = self.update_arousal();
            let vma = self.valence.moving_average();
            let ama = self.arousal.moving_average();

            normalized_valence_option = self.valence.normalize(vma);
            normalized_arousal_option = self.arousal.normalize(ama);
        }

        (normalized_valence_option, normalized_arousal_option)
    }

    /// Front assymetry- higher values mean more positive mood
    fn front_assymetry(&self) -> f32 {
        E.powf(self.alpha[AF8] - self.alpha[AF7])
    }

    /// Positive-negative balance of emotion
    pub fn calc_absolute_valence(&self) -> f32 {
        self.front_assymetry() / average_from_front_electrodes(&self.theta)
    }

    /// Level of emotional intensity based on other, more primitive values
    pub fn calc_abolute_arousal(&self) -> f32 {
        let frontal_apha = (E.powf(self.alpha[AF7]) + E.powf(self.alpha[AF8])) / 2.0;
        let frontal_theta = (E.powf(self.theta[AF7]) + E.powf(self.theta[AF8])) / 2.0;
        frontal_theta / (frontal_apha + 1e-6)
    }

    /// Calculate the current arousal value and add it to the length-limited history
    pub fn update_arousal(&mut self) -> bool {
        let abs_arousal = self.calc_abolute_arousal();
        self.arousal.set(abs_arousal)
    }

    /// Calculate the current valence value and add it to the length-limited history
    pub fn update_valence(&mut self) -> bool {
        let abs_valence = self.calc_absolute_valence();
        self.valence.set(abs_valence)
    }

    /// Update state based on an incoming message
    fn handle_muse_message(
        &mut self,
        muse_message: MuseMessage,
    ) -> Result<bool, SendError<TimedMuseMessage>> {
        let message_time = muse_message.message_time;

        match muse_message.muse_message_type {
            MuseMessageType::Accelerometer { x, y, z } => {
                self.accelerometer = [x, y, z];
                self.log_other(message_time, &format!("Accel, {:?}, {:?}, {:?}", x, y, z));
                Ok(false)
            }
            MuseMessageType::Gyro { x, y, z } => {
                self.gyro = [x, y, z];
                self.log_other(message_time, &format!("Gyro, {:?}, {:?}, {:?}", x, y, z));
                Ok(false)
            }
            MuseMessageType::Horseshoe { a, b, c, d } => {
                self.horseshoe = [a, b, c, d];
                self.log_other(
                    message_time,
                    &format!("Horseshoe, {:?}, {:?}, {:?}, {:?}", a, b, c, d),
                );
                // self.send((time, MuseMessageType::Horseshoe { a, b, c, d }));
                Ok(false)
            }
            MuseMessageType::Eeg { .. } => {
                self.eeg_log_sender
                    .send(muse_message)
                    .expect("Unable to log eeg");
                Ok(false)
            }
            MuseMessageType::Alpha { alpha } => {
                self.alpha = alpha;
                self.alpha_log_sender
                    .send(muse_message)
                    .expect("Unable to log alpha");
                Ok(true)
            }
            MuseMessageType::Beta { beta } => {
                self.beta = beta;
                self.beta_log_sender
                    .send(muse_message)
                    .expect("Unable to log beta");
                Ok(true)
            }
            MuseMessageType::Gamma { gamma } => {
                self.gamma = gamma;
                self.gamma_log_sender
                    .send(muse_message)
                    .expect("Unable to log gamma");
                Ok(true)
            }
            MuseMessageType::Delta { a, b, c, d } => {
                self.delta = [a, b, c, d];
                self.log_delta(message_time);
                Ok(true)
            }
            MuseMessageType::Theta { a, b, c, d } => {
                self.theta = [a, b, c, d];
                self.log_theta(message_time);
                Ok(true)
            }
            MuseMessageType::Batt { batt } => {
                self.batt = batt;
                self.log_other(message_time, &format!("Battery, {:?}", batt));
                Ok(false)
            }
            MuseMessageType::TouchingForehead { touch } => {
                let mut i = 0;
                if touch {
                    i = 1;
                } else {
                    self.touching_forehead_countdown = FOREHEAD_COUNTDOWN;
                };
                self.log_other(message_time, &format!("Battery, {:?}", i));
                Ok(false)
            }
            MuseMessageType::Blink { blink } => {
                let mut i = 0;
                if blink {
                    self.blink_countdown = BLINK_COUNTDOWN;
                    i = 1;
                };
                self.log_other(message_time, &format!("Blink, {:?}", i));
                //                self.send((time, MuseMessageType::Blink { blink }));
                Ok(false)
            }
            MuseMessageType::JawClench { clench } => {
                let mut i = 0;
                if clench {
                    self.jaw_clench_countdown = CLENCH_COUNTDOWN;
                    i = 1;
                };
                self.log_other(message_time, &format!("Clench, {:?}", i));
                // self.send((time, MuseMessageType::JawClench { clench }));
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_mean() {
        let v: Vec<f64> = Vec::new();
        assert_eq!(None, crate::muse_model::mean(&v));
    }

    #[test]
    fn test_mean() {
        let mut v = vec![1.0, 3.0];
        assert_eq!(2.0, crate::muse_model::mean(&v).unwrap());

        v.push(5.0);
        assert_eq!(3.0, crate::muse_model::mean(&v).unwrap());
    }

    #[test]
    fn test_no_deviation() {
        let v: Vec<f64> = Vec::new();
        let mean = crate::muse_model::mean(&v);
        assert_eq!(None, crate::muse_model::std_deviation(&v, mean));
    }

    #[test]
    fn test_deviation() {
        let mut v = vec![1.0];
        let mut mean = crate::muse_model::mean(&v);
        assert_eq!(0.0, crate::muse_model::std_deviation(&v, mean).unwrap());

        v.push(3.0);
        mean = crate::muse_model::mean(&v);
        assert_eq!(1.0, crate::muse_model::std_deviation(&v, mean).unwrap());

        v.push(5.0);
        v.push(7.0);
        mean = crate::muse_model::mean(&v);
        assert_eq!(
            2.23606797749979,
            crate::muse_model::std_deviation(&v, mean).unwrap()
        );
    }

    #[test]
    fn test_new_normalized_value() {
        let nv: NormalizedValue<f32> = NormalizedValue::new();

        assert_eq!(nv.current, None);
        assert_eq!(nv.min, None);
        assert_eq!(nv.max, None);
        assert_eq!(nv.moving_average(), None);
        assert_eq!(nv.deviation, None);
        assert_eq!(nv._percent(), None);
        assert_eq!(nv.normalize(nv.moving_average()), None);
        assert_eq!(nv.history.len(), 0);
    }

    #[test]
    fn test_single_normalized_value() {
        let mut nv: NormalizedValue<f64> = NormalizedValue::new();
        nv.set(1.0);

        assert_eq!(nv.current, Some(1.0));
        assert_eq!(nv.min, Some(1.0));
        assert_eq!(nv.max, Some(1.0));
        assert_eq!(nv.moving_average(), Some(1.0));
        assert_eq!(nv.mean(), Some(1.0));
        assert_eq!(nv.deviation(), Some(0.0));
        assert_eq!(nv._percent(), Some(0.0));
        assert_eq!(nv.history.len(), 1);
    }

    #[test]
    fn test_two_normalized_values_second_normalized() {
        let mut nv: NormalizedValue<f64> = NormalizedValue::new();
        nv.set(1.0);
        nv.set(3.0);

        assert_eq!(nv.current, Some(3.0));
        assert_eq!(nv.min, Some(1.0));
        assert_eq!(nv.max, Some(3.0));
        assert_eq!(nv.moving_average(), Some(2.0));
        assert_eq!(nv.mean(), Some(2.0));
        assert_eq!(nv.deviation(), Some(1.0));
        assert_eq!(nv.history.len(), 2);
    }

    #[test]
    fn test_normalized_value_history() {
        const LENGTH: usize = 120;
        let mut nv: NormalizedValue<f64> = NormalizedValue::new();

        for i in 0..LENGTH {
            nv.set(i as f64);
        }

        assert_eq!(nv.min, Some(0.0));
        assert_eq!(nv.max, Some((LENGTH - 1) as f64));
        assert_eq!(nv.moving_average(), Some(115.0));
        assert_eq!(nv.mean(), Some(59.5));
        assert_eq!(nv.deviation(), Some(34.63981331743384));
        assert_eq!(nv.normalize(nv.moving_average()), Some(1.602202630002843));
        assert_eq!(nv.normalize(nv.min), Some(-1.7176766934264711));
        assert_eq!(nv.normalize(nv.max), Some(1.7176766934264711));
        assert_eq!(nv.history.len(), 120);
        assert_eq!(nv._percent_normalization_complete(), 1.0);
    }

    #[test]
    fn test_normalized_value_history_with_negative_values() {
        let mut nv: NormalizedValue<f32> = NormalizedValue::new();

        // Twice as many values as the initial normalization stage. All normlalization values are negative
        for i in -100..101 {
            nv.set(i as f32);
        }

        assert_eq!(nv.min, Some(-100.0));
        assert_eq!(nv.max, Some(100.0));
        assert_eq!(nv.moving_average(), Some(96.0));
        assert_eq!(nv.mean(), Some(40.5));
        assert_eq!(nv.deviation(), Some(34.63981331743384));
        assert_eq!(nv.normalize(nv.moving_average()), Some(1.6022027));
        assert_eq!(nv.normalize(nv.min), Some(-4.0560265));
        assert_eq!(nv.normalize(nv.max), Some(1.7176768));
        assert_eq!(nv.history.len(), 120);
    }

    #[test]
    fn test_current_time_formatting_for_filenames() {
        let current_time = Local::now();
        let s = date_time_filename_format(current_time);
        println!("{}", s);

        assert_eq!(23, s.len());
    }

    #[test]
    fn test_current_time_formatting_for_csv() {
        let current_time = Local::now();
        let s = date_time_csv_format(current_time);
        println!("{}", s);

        assert_eq!(23, s.len());
    }
}
