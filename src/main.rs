/// Run a test where a Muse headset collects EEG data based on a series of
/// images presented to the wearer. Push that raw collected data to a Postgresql database.
#[macro_use]
extern crate log;

// Draw some multi-colored geometry to the screen
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
extern crate env_logger;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
extern crate web_logger;

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
extern crate nannou_osc;

extern crate arr_macro;
extern crate chrono;
extern crate mandala;
extern crate num_traits;
extern crate quicksilver;

use crate::eeg_view::ImageSet;
use arr_macro::arr;
use csv::Writer;
use eeg_view::EegViewState;
use log::{error, info};
use mandala::{Mandala, MandalaState};
use muse_model::{DisplayType, MuseModel};
use quicksilver::{
    combinators::result,
    geom::{Line, Rectangle, Shape, Transform, Vector},
    graphics::{Background::Img, Color, Font, FontStyle, Image, Mesh, ShapeRenderer},
    input::{ButtonState, GamepadButton, Key, MouseButton},
    lifecycle::{run, Asset, Event, Settings, State, Window},
    sound::Sound,
    Future, Result,
};
use std::fs::File;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

mod eeg_view;
mod muse_model;

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod muse_packet;

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
const SCREEN_SIZE: (f32, f32) = (1920.0, 1200.0);
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
const SCREEN_SIZE: (f32, f32) = (1280.0, 650.0);
const IMAGE_DURATION_FRAMES: u64 = 300;
const INTER_IMAGE_INTERVAL: u64 = 90;
const _IMAGE_SET_SIZE: usize = 24;
const MANDALA_CENTER: (f32, f32) = (SCREEN_SIZE.0 / 2.0, SCREEN_SIZE.1 / 2.0);
const MANDALA_SCALE: (f32, f32) = (3.0, 3.0); // Adjust size of Mandala vs screen

const FPS: u64 = 60; // Frames per second
const UPS: u64 = 60; // Updates per second
const TITLE: u64 = 4 * FPS;
const INTRO_A: u64 = TITLE + 25 * FPS; // INTRO
const INTRO_B: u64 = INTRO_A + 6 * FPS;
const INTRO_C: u64 = INTRO_B + 8 * FPS;
const NEGATIVE_A: u64 = INTRO_C + 22 * FPS; // TASK 1
const NEGATIVE_B: u64 = NEGATIVE_A + 116 * FPS;
const BREATHING_A: u64 = NEGATIVE_B + 10 * FPS; // TASK 2
const BREATHING_B: u64 = BREATHING_A + 120 * FPS;
const POSITIVE_A: u64 = BREATHING_B + 19 * FPS; // TASK 3
const POSITIVE_B: u64 = POSITIVE_A + 120 * FPS;
const FREE_RIDE_AA: u64 = POSITIVE_B + (0.5 * FPS as f32) as u64; // TASK 4
const FREE_RIDE_AB: u64 = FREE_RIDE_AA + (0.5 * FPS as f32) as u64;
const FREE_RIDE_AC: u64 = FREE_RIDE_AB + (0.5 * FPS as f32) as u64;
const FREE_RIDE_AD: u64 = FREE_RIDE_AC + (0.5 * FPS as f32) as u64;
const FREE_RIDE_AE: u64 = FREE_RIDE_AD + 17 * FPS;
const FREE_RIDE_B: u64 = FREE_RIDE_AE + 70 * FPS; // (same image)
const FREE_RIDE_C: u64 = FREE_RIDE_B + 10 * FPS; // (same image)
const THANK_YOU: u64 = FREE_RIDE_C + 9 * FPS; // THANK YOU

const IMAGE_LOGO: &str = "Nof1-logo.png";
const MANDALA_VALENCE_PETAL_SVG_NAME: &str = "mandala_valence_petal.svg";
const MANDALA_AROUSAL_PETAL_SVG_NAME: &str = "mandala_arousal_petal.svg";
const MANDALA_BREATH_PETAL_SVG_NAME: &str = "mandala_breath_petal.svg";
/// The visual slew time from current value to newly set value. Keep in mind that the newly set value is already smoothed, so this number should be small to provide consinuous interpolation between new values, not large to provide an additional layer of (less carefully controlled) smoothing filter.
const MANDALA_TRANSITION_DURATION: f32 = 0.5;

const FONT_EXTRA_BOLD: &str = "WorkSans-ExtraBold.ttf";
const FONT_MULI: &str = "Muli.ttf";
const FONT_EXTRA_BOLD_SIZE: f32 = 72.0;
const FONT_MULI_SIZE: f32 = 40.0;
const FONT_GRAPH_LABEL_SIZE: f32 = 40.0;
const FONT_EEG_LABEL_SIZE: f32 = 30.0;

const SOUND_CLICK: &str = "click.ogg";
const _SOUND_GUIDANCE: &str = "Meet Your Mind Leo's voice 200224.mp3";

const STR_TITLE: &str = "Meme Machine";
const STR_HELP_TEXT: &str = "First relax and watch your mind calm\n\nYou will then be shown some images. Press the left and right images to tell us if they are\nfamiliar and how they make you feel.";

const _COLOR_GREY: Color = Color {
    r: 0.5,
    g: 0.5,
    b: 0.5,
    a: 1.0,
};
const COLOR_CLEAR: Color = Color {
    r: 0.5,
    g: 0.5,
    b: 0.5,
    a: 0.0,
};
const COLOR_NOF1_DARK_BLUE: Color = Color {
    r: 31. / 256.,
    g: 18. / 256.,
    b: 71. / 256.,
    a: 1.0,
};
const COLOR_NOF1_LIGHT_BLUE: Color = Color {
    r: 189. / 256.,
    g: 247. / 256.,
    b: 255. / 256.,
    a: 1.0,
};
const COLOR_NOF1_TURQOISE: Color = Color {
    r: 0. / 256.,
    g: 200. / 256.,
    b: 200. / 256.,
    a: 1.0,
};
const COLOR_BACKGROUND: Color = Color::BLACK;
const COLOR_TITLE: Color = COLOR_NOF1_DARK_BLUE;
const COLOR_EEG_LABEL: Color = COLOR_NOF1_DARK_BLUE;
const COLOR_TEXT: Color = Color::BLACK;
const _COLOR_BUTTON: Color = COLOR_NOF1_DARK_BLUE;
const COLOR_BUTTON_PRESSED: Color = COLOR_NOF1_LIGHT_BLUE;
const COLOR_EMOTION: Color = Color::YELLOW;
const COLOR_VALENCE_MANDALA_CLOSED: Color = Color {
    // Purple, positive
    r: 0.415,
    g: 0.051,
    b: 0.67,
    a: 0.8,
};

const COLOR_VALENCE_MANDALA_OPEN: Color = Color {
    // Crimson, negative
    r: 220.0 / 256.0,
    g: 20.0 / 256.0,
    b: 60.0 / 256.0,
    a: 0.85,
};
const COLOR_AROUSAL_MANDALA_CLOSED: Color = Color {
    //Blue, low arousal
    r: 189. / 256.,
    g: 247. / 256.,
    b: 255. / 256.,
    a: 0.7,
};
const COLOR_AROUSAL_MANDALA_OPEN: Color = Color {
    // yellow orange, Low arousal 255, 174, 66
    r: 255.0 / 256.0,
    g: 174.0 / 256.0,
    b: 66.0 / 256.0,
    a: 1.0,
};
const COLOR_BREATH_MANDALA_CLOSED: Color = Color {
    //Blue, transparent, breath out
    r: 10. / 256.,
    g: 10. / 256.,
    b: 256. / 256.,
    a: 0.9,
};
const COLOR_BREATH_MANDALA_OPEN: Color = Color {
    // Green opaque, breath in
    r: 10.0 / 256.0,
    g: 256.0 / 256.0,
    b: 10.0 / 256.0,
    a: 0.0,
};

const BUTTON_WIDTH: f32 = 200.0;
const BUTTON_HEIGHT: f32 = 50.0;
const BUTTON_H_MARGIN: f32 = 20.0;
const BUTTON_V_MARGIN: f32 = 20.0;

const TITLE_V_MARGIN: f32 = 40.0;
const TEXT_V_MARGIN: f32 = 200.0;

const RECT_LEFT_BUTTON: Rectangle = Rectangle {
    pos: Vector {
        x: BUTTON_H_MARGIN,
        y: SCREEN_SIZE.1 - BUTTON_V_MARGIN - BUTTON_HEIGHT,
    },
    size: Vector {
        x: BUTTON_WIDTH,
        y: BUTTON_HEIGHT,
    },
};

const RECT_RIGHT_BUTTON: Rectangle = Rectangle {
    pos: Vector {
        x: SCREEN_SIZE.0 - BUTTON_H_MARGIN - BUTTON_WIDTH,
        y: SCREEN_SIZE.1 - BUTTON_V_MARGIN - BUTTON_HEIGHT,
    },
    size: Vector {
        x: BUTTON_WIDTH,
        y: BUTTON_HEIGHT,
    },
};

pub trait OscSocket: Sized {
    fn osc_socket_receive();
}

struct AppState {
    frame_count: u64,
    start_time: Instant,
    title_text: Asset<Image>,
    help_text: Asset<Image>,
    logo: Asset<Image>,
    sound_click: Asset<Sound>,
    sound_e1: Asset<Sound>,
    sound_e2: Asset<Sound>,
    sound_e3: Asset<Sound>,
    sound_e4: Asset<Sound>,
    sound_e5: Asset<Sound>,
    sound_e6: Asset<Sound>,
    sound_e7: Asset<Sound>,
    sound_e8: Asset<Sound>,
    sound_e9: Asset<Sound>,
    help_1: Asset<Image>,
    help_2: Asset<Image>,
    help_3: Asset<Image>,
    help_4: Asset<Image>,
    help_5: Asset<Image>,
    help_6: Asset<Image>,
    help_7a: Asset<Image>,
    help_7b: Asset<Image>,
    help_7c: Asset<Image>,
    help_8: Asset<Image>,
    help_9: Asset<Image>,
    left_button_color: Color,
    right_button_color: Color,
    mandala_valence: Mandala,
    mandala_arousal: Mandala,
    mandala_breath: Mandala,
    muse_model: MuseModel,
    eeg_view_state: EegViewState,
    _rx_eeg: Receiver<(Duration, muse_model::MuseMessageType)>,
    positive_images: ImageSet,
    negative_images: ImageSet,
    image_index_positive: usize,
    image_index_negative: usize,
    local_frame: u64,
    mandala_on: bool,
}

fn breathing_sinusoid_10sec(current_time: f32) -> f32 {
    let pi: f32 = std::f32::consts::PI;
    let sin: f32 = (current_time * 0.2f32 * pi).sin();
    sin / 2.0f32 + 0.5f32
}

impl AppState {
    // Perform any shutdown actions
    // Do not call this directly to end the app. Instead call window.close();
    fn shutdown_hooks(&mut self) -> Result<()> {
        // TODO Notify database session ended

        Ok(())
    }

    fn left_action(&mut self, _window: &mut Window) -> Result<()> {
        self.left_button_color = COLOR_BUTTON_PRESSED;
        self.sound_click
            .execute(|sound| sound.play())
            .expect("Could not play left button sound");
        Ok(())
    }

    fn right_action(&mut self, _window: &mut Window) -> Result<()> {
        self.right_button_color = COLOR_BUTTON_PRESSED;
        self.sound_click.execute(|sound| sound.play())
    }
}

impl AppState {
    fn seconds_since_start(&self) -> f32 {
        self.start_time.elapsed().as_nanos() as f32 / 1000000000.0
    }

    fn draw_mandala(&mut self, mandala_on: bool, window: &mut Window) {
        if !mandala_on {
            return;
        }
        let mut mesh = Mesh::new();

        let mut shape_renderer = ShapeRenderer::new(&mut mesh, Color::RED);
        let seconds_since_start = self.seconds_since_start();
        self.mandala_valence
            .draw(seconds_since_start, &mut shape_renderer);
        self.mandala_arousal
            .draw(seconds_since_start, &mut shape_renderer);
        window.mesh().extend(&mesh);
    }

    fn draw_breath_mandala(&mut self, current_time: f32, window: &mut Window) {
        let mut mesh = Mesh::new();
        let breath_state = breathing_sinusoid_10sec(current_time);
        let mut shape_renderer = ShapeRenderer::new(&mut mesh, Color::RED);
        let seconds_since_start = self.seconds_since_start();
        self.mandala_breath
            .start_transition(current_time, 0.01, breath_state);
        self.mandala_breath
            .draw(seconds_since_start, &mut shape_renderer);
        window.mesh().extend(&mesh);
    }
}

#[allow(dead_code)]
fn bound_normalized_value(normalized: f32) -> f32 {
    normalized.max(3.0).min(-3.0)
}

/// Create a log of values and events collected during a session
fn create_log_writer(filename: &str) -> Writer<File> {
    let current_date_time = muse_model::current_date_time_filename_format();
    let filename_with_date_time = format!("{} {}", current_date_time, filename);
    let writer: Writer<File> =
        Writer::from_path(filename_with_date_time).expect("Could not open CSV file for writing");

    writer
}

impl State for AppState {
    fn new() -> Result<AppState> {
        let title_font = Font::load(FONT_EXTRA_BOLD);
        let help_font = Font::load(FONT_MULI);
        let title_text = Asset::new(title_font.and_then(|font| {
            result(font.render(
                STR_TITLE,
                &FontStyle::new(FONT_EXTRA_BOLD_SIZE, COLOR_TITLE),
            ))
        }));
        let help_text = Asset::new(help_font.and_then(|font| {
            result(font.render(STR_HELP_TEXT, &FontStyle::new(FONT_MULI_SIZE, COLOR_TEXT)))
        }));

        let logo = Asset::new(Image::load(IMAGE_LOGO));
        let sound_click = Asset::new(Sound::load(SOUND_CLICK));
        let sound_e1 = Asset::new(Sound::load("F1.mp3"));
        let sound_e2 = Asset::new(Sound::load("F2.mp3"));
        let sound_e3 = Asset::new(Sound::load("F3.mp3"));
        let sound_e4 = Asset::new(Sound::load("F4.mp3"));
        let sound_e5 = Asset::new(Sound::load("F5.mp3"));
        let sound_e6 = Asset::new(Sound::load("F6.mp3"));
        let sound_e7 = Asset::new(Sound::load("F7.mp3"));
        let sound_e8 = Asset::new(Sound::load("F8.mp3"));
        let sound_e9 = Asset::new(Sound::load("F9.mp3"));

        let help_1 = Asset::new(Image::load("1fi.png"));
        let help_2 = Asset::new(Image::load("2fi.png"));
        let help_3 = Asset::new(Image::load("3fi.png"));
        let help_4 = Asset::new(Image::load("4fi.png"));
        let help_5 = Asset::new(Image::load("5fi.png"));
        let help_6 = Asset::new(Image::load("6fi.png"));
        let help_7a = Asset::new(Image::load("7a_fi.png"));
        let help_7b = Asset::new(Image::load("7b_fi.png"));
        let help_7c = Asset::new(Image::load("7c_fi.png"));
        let help_8 = Asset::new(Image::load("8fi.png"));
        let help_9 = Asset::new(Image::load("9fi.png"));

        //        let sound_blah = Asset::new(Sound::load(SOUND_GUIDANCE));
        let (rx_eeg, muse_model) = muse_model::MuseModel::new();
        let mandala_valence_state_open = MandalaState::new(
            COLOR_VALENCE_MANDALA_OPEN,
            Transform::rotate(90),
            Transform::translate((50.0, 0.0)),
            Transform::scale((0.85, 0.95)),
        );
        let mandala_valence_state_closed = MandalaState::new(
            COLOR_VALENCE_MANDALA_CLOSED,
            Transform::rotate(0.0),
            Transform::translate((0.0, 0.0)),
            Transform::scale((0.8, 0.65)),
        );
        let mut mandala_valence = Mandala::new(
            MANDALA_VALENCE_PETAL_SVG_NAME,
            MANDALA_CENTER,
            MANDALA_SCALE,
            12,
            mandala_valence_state_open,
            mandala_valence_state_closed,
            1.0,
        );
        let mandala_arousal_state_open = MandalaState::new(
            COLOR_AROUSAL_MANDALA_OPEN,
            Transform::rotate(60),
            Transform::translate((35.0, 0.0)),
            Transform::scale((0.85, 0.75)),
        );
        let mandala_arousal_state_closed = MandalaState::new(
            COLOR_AROUSAL_MANDALA_CLOSED,
            Transform::rotate(0.0),
            Transform::translate((0.0, 0.0)),
            Transform::scale((1., 1.)),
        );
        let mandala_breath_state_open = MandalaState::new(
            COLOR_BREATH_MANDALA_OPEN,
            Transform::rotate(30),
            Transform::translate((45.0, 0.0)),
            Transform::scale((1.0, 0.50)),
        );
        let mandala_breath_state_closed = MandalaState::new(
            COLOR_BREATH_MANDALA_CLOSED,
            Transform::rotate(0.0),
            Transform::translate((0.0, 0.0)),
            Transform::scale((0.3, 0.1)),
        );
        let mut mandala_arousal = Mandala::new(
            MANDALA_AROUSAL_PETAL_SVG_NAME,
            MANDALA_CENTER,
            MANDALA_SCALE,
            12,
            mandala_arousal_state_open,
            mandala_arousal_state_closed,
            0.0,
        );
        let mandala_breath = Mandala::new(
            MANDALA_BREATH_PETAL_SVG_NAME,
            MANDALA_CENTER,
            MANDALA_SCALE,
            12,
            mandala_breath_state_open,
            mandala_breath_state_closed,
            0.0,
        );
        mandala_valence.start_transition(0.0, 3.0, 0.0);
        mandala_arousal.start_transition(0.0, 3.0, 1.0);

        let eeg_view_state = EegViewState::new();
        let start_time = Instant::now();
        //println!("Start instant: {:?}", start_time);
        let positive_images = ImageSet::new(r#"positive-images//p"#);
        let negative_images = ImageSet::new(r#"negative-images//n"#);
        let image_index_positive: usize = 0;
        let image_index_negative: usize = 0;
        let local_frame: u64 = 0;
        let mandala_on = true;

        Ok(AppState {
            frame_count: 0,
            start_time,
            title_text,
            help_text,
            logo,
            sound_click,
            mandala_valence,
            mandala_arousal,
            mandala_breath,
            sound_e1,
            sound_e2,
            sound_e3,
            sound_e4,
            sound_e5,
            sound_e6,
            sound_e7,
            sound_e8,
            sound_e9,
            help_1,
            help_2,
            help_3,
            help_4,
            help_5,
            help_6,
            help_7a,
            help_7b,
            help_7c,
            help_8,
            help_9,
            left_button_color: COLOR_CLEAR,
            right_button_color: COLOR_CLEAR,
            eeg_view_state,
            _rx_eeg: rx_eeg,
            muse_model,
            positive_images,
            negative_images,
            image_index_positive,
            image_index_negative,
            local_frame,
            mandala_on,
        })
    }

    // This is called UPS times per second
    fn update(&mut self, window: &mut Window) -> Result<()> {
        // EXIT APP
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        {
            if window.keyboard()[Key::Escape].is_down()
                || window
                    .gamepads()
                    .iter()
                    .any(|pad| pad[GamepadButton::FaceLeft].is_down())
            {
                self.muse_model.flush_all();
                window.close();
            }
        }

        // LEFT SHIFT OR GAMEPAD ACTION
        if window.keyboard()[Key::LShift] == ButtonState::Pressed
            || window
                .gamepads()
                .iter()
                .any(|pad| pad[GamepadButton::TriggerLeft].is_down())
            || window
                .gamepads()
                .iter()
                .any(|pad| pad[GamepadButton::ShoulderLeft].is_down())
        {
            self.left_action(window)?;
        }

        // RIGHT SHIFT OR GAMEPAD ACTION
        if window.keyboard()[Key::RShift] == ButtonState::Pressed
            || window
                .gamepads()
                .iter()
                .any(|pad| pad[GamepadButton::TriggerRight].is_down())
            || window
                .gamepads()
                .iter()
                .any(|pad| pad[GamepadButton::ShoulderRight].is_down())
        {
            self.right_action(window)?;
        }

        // LEFT SCREEN BUTTON PRESS
        if window.mouse()[MouseButton::Left] == ButtonState::Pressed
            && RECT_LEFT_BUTTON.contains(window.mouse().pos())
        {
            self.left_action(window)?;
        }

        // RIGHT SCREEN BUTTON PRESS
        if window.mouse()[MouseButton::Left] == ButtonState::Pressed
            && RECT_RIGHT_BUTTON.contains(window.mouse().pos())
        {
            self.right_action(window)?;
        }

        // TODO NANO SEEED BUTTON PRESS

        // F1
        if window.keyboard()[Key::F1] == ButtonState::Pressed {
            self.muse_model.display_type = DisplayType::Mandala;
        }

        // F2
        if window.keyboard()[Key::F2] == ButtonState::Pressed {
            self.muse_model.display_type = DisplayType::Dowsiness;
        }

        // F3
        if window.keyboard()[Key::F3] == ButtonState::Pressed {
            self.muse_model.display_type = DisplayType::Emotion;
        }

        // F4
        if window.keyboard()[Key::F4] == ButtonState::Pressed {
            self.muse_model.display_type = DisplayType::EegValues;
        }

        let (normalized_valence_option, normalized_arousal_option) =
            self.muse_model.receive_packets();
        if self.frame_count > TITLE {
            let current_time = self.seconds_since_start();
            if let Some(normalized_valence) = normalized_valence_option {
                if normalized_valence.is_finite() {
                    self.mandala_valence.start_transition(
                        current_time,
                        MANDALA_TRANSITION_DURATION,
                        normalized_valence,
                    );
                }
            }
            if let Some(normalized_arousal) = normalized_arousal_option {
                if normalized_arousal.is_finite() {
                    self.mandala_arousal.start_transition(
                        current_time,
                        MANDALA_TRANSITION_DURATION,
                        normalized_arousal,
                    );
                }
            }
        }
        self.muse_model.count_down();

        Ok(())
    }

    fn event(&mut self, event: &Event, _window: &mut Window) -> Result<()> {
        if let Event::Closed = event {
            self.shutdown_hooks()?;
        }

        Ok(())
    }

    // This is called FPS times per second
    fn draw(&mut self, window: &mut Window) -> Result<()> {
        let background_color = COLOR_BACKGROUND;
        window.clear(background_color)?;

        // THE NAME AT THE TOP OF THE IF STATEMENT IS THE NAME OF THE PREVIOUS STAGE
        if self.frame_count == TITLE {
            let _result = self.sound_e1.execute(|sound| sound.play());
        }
        if self.frame_count == INTRO_C {
            let _result = self.sound_e2.execute(|sound| sound.play());
        }
        if self.frame_count == NEGATIVE_A {
            let _result = self.sound_e3.execute(|sound| sound.play());
        }
        if self.frame_count == NEGATIVE_B {
            let _result = self.sound_e4.execute(|sound| sound.play());
        }
        if self.frame_count == BREATHING_B {
            let _result = self.sound_e5.execute(|sound| sound.play());
        }
        if self.frame_count == POSITIVE_A {
            let _result = self.sound_e6.execute(|sound| sound.play());
        }
        if self.frame_count == POSITIVE_B {
            let _result = self.sound_e7.execute(|sound| sound.play());
        }
        if self.frame_count == FREE_RIDE_AA {
            let _result = self.sound_e8.execute(|sound| sound.play());
        }
        if self.frame_count == FREE_RIDE_C {
            let _result = self.sound_e9.execute(|sound| sound.play());
        }

        let optional_image: Option<&mut Asset<Image>> =
            if self.frame_count > TITLE && self.frame_count < INTRO_A {
                Some(&mut self.help_1)
            } else if self.frame_count >= INTRO_A && self.frame_count < INTRO_B {
                Some(&mut self.help_2)
            } else if self.frame_count >= INTRO_B && self.frame_count < INTRO_C {
                Some(&mut self.help_3)
            } else if self.frame_count >= INTRO_C && self.frame_count < NEGATIVE_A {
                Some(&mut self.help_4)
            // } else if self.frame_count >= NEGATIVE_A && self.frame_count < NEGATIVE_B {
            //     Some(&mut self.help_5)
            } else if self.frame_count >= NEGATIVE_B && self.frame_count < BREATHING_A {
                Some(&mut self.help_5)
            // } else if self.frame_count >= BREATHING_A && self.frame_count < BREATHING_B {
            //     Some(&mut self.help_)
            } else if self.frame_count >= BREATHING_B && self.frame_count < POSITIVE_A {
                Some(&mut self.help_6)
            // } else if self.frame_count >= POSITIVE_A && self.frame_count < POSITIVE_B {
            //     Some(&mut self.help_)
            // } else if self.frame_count >= POSITIVE_B && self.frame_count < FREE_RIDE_A {
            //     Some(&mut self.help_)
            // } else if self.frame_count >= FREE_RIDE_AA && self.frame_count < FREE_RIDE_AB {
            //     Some(&mut self.help_7a)
            } else if self.frame_count >= FREE_RIDE_AB && self.frame_count < FREE_RIDE_AC {
                Some(&mut self.help_7a)
            } else if self.frame_count >= FREE_RIDE_AC && self.frame_count < FREE_RIDE_AD {
                Some(&mut self.help_7b)
            } else if self.frame_count >= FREE_RIDE_AD && self.frame_count < FREE_RIDE_B {
                Some(&mut self.help_7c)
            } else if self.frame_count >= FREE_RIDE_B && self.frame_count < FREE_RIDE_C {
                Some(&mut self.help_8)
            } else if self.frame_count >= FREE_RIDE_C {
                Some(&mut self.help_9)
            } else {
                None
            };

        match optional_image {
            Some(i) => {
                i.execute(|image| {
                    window.draw(
                        &image
                            .area()
                            .with_center((SCREEN_SIZE.0 / 2.0, SCREEN_SIZE.1 / 4.0)),
                        Img(&image),
                    );
                    Ok(())
                })?;
            }
            None => (),
        }

        if self.frame_count < TITLE {
            self.draw_mandala(self.mandala_on, window);

            // LOGO
            self.logo.execute(|image| {
                window.draw(
                    &image
                        .area()
                        .with_center((SCREEN_SIZE.0 / 2.0, SCREEN_SIZE.1 / 4.0)),
                    Img(&image),
                );
                Ok(())
            })?;
        }; //else if self.frame_count < INTRO_A {
           // self.help_1.execute(|image| {
           //     window.draw(
           //         &image
           //             .area()
           //             .with_center((SCREEN_SIZE.0 / 2.0, SCREEN_SIZE.1 / 4.0)),
           //         Img(&image),
           //     );
           //     Ok(())
           // })?;

        // TITLE
        // self.title_text.execute(|image| {
        //     window.draw(
        //         &image
        //             .area()
        //             .with_center((SCREEN_SIZE.0 / 2.0, TITLE_V_MARGIN)),
        //         Img(&image),
        //     );
        //     Ok(())
        // })?;

        // // TEXT
        // self.help_text.execute(|image| {
        //     window.draw(
        //         &image
        //             .area()
        //             .with_center((SCREEN_SIZE.0 / 2.0, TEXT_V_MARGIN)),
        //         Img(&image),
        //     );
        //     Ok(())
        // })?;

        // RIGHT BUTTON
        // let right_color = self.right_button_color;
        // self.sound_click.execute(|_| {
        //     window.draw(&RECT_RIGHT_BUTTON, Col(right_color));
        //     Ok(())
        // })?;
        // self.right_button_color = COLOR_BUTTON;
        if self.frame_count > NEGATIVE_A && self.frame_count < NEGATIVE_B {
            match self.muse_model.display_type {
                DisplayType::Mandala => {
                    self.draw_mandala(self.mandala_on, window);
                    if self.local_frame < IMAGE_DURATION_FRAMES {
                        self.negative_images.draw(self.image_index_negative, window);
                        self.local_frame += 1;
                    } else if self.local_frame < IMAGE_DURATION_FRAMES + INTER_IMAGE_INTERVAL {
                        //TODO Interstitial interval
                        self.local_frame += 1;
                    } else {
                        self.mandala_on = true;
                        //println!("ELSE: {}", self.local_frame);
                        self.local_frame *= 0;
                        self.image_index_negative += 1 as usize;
                    }
                }

                _ => eeg_view::draw_view(&self.muse_model, window, &mut self.eeg_view_state),
            }
        };

        if self.frame_count > BREATHING_A && self.frame_count < BREATHING_B {
            self.mandala_on = false;
            match self.muse_model.display_type {
                DisplayType::Mandala => {
                    self.draw_mandala(self.mandala_on, window);
                    // println!("Breathing block!");
                    self.draw_breath_mandala(self.seconds_since_start(), window);
                    self.mandala_on = true;
                    self.local_frame = 0;
                }
                _ => eeg_view::draw_view(&self.muse_model, window, &mut self.eeg_view_state),
            }
        };

        if self.frame_count > POSITIVE_A && self.frame_count < POSITIVE_B {
            match self.muse_model.display_type {
                DisplayType::Mandala => {
                    self.draw_mandala(self.mandala_on, window);
                    if self.local_frame < IMAGE_DURATION_FRAMES {
                        self.positive_images.draw(self.image_index_positive, window);
                        self.local_frame += 1;
                    } else if self.local_frame < IMAGE_DURATION_FRAMES + INTER_IMAGE_INTERVAL {
                        //TODO Interstitial interval
                        self.local_frame += 1;
                    } else {
                        self.mandala_on = true;
                        //println!("ELSE: {}", self.local_frame);
                        self.local_frame *= 0;
                        self.image_index_positive += 1 as usize;
                    }
                }

                _ => eeg_view::draw_view(&self.muse_model, window, &mut self.eeg_view_state),
            }
        };

        if self.frame_count > FREE_RIDE_AA && self.frame_count < FREE_RIDE_C {
            match self.muse_model.display_type {
                DisplayType::Mandala => {
                    self.draw_mandala(self.mandala_on, window);
                }
                _ => eeg_view::draw_view(&self.muse_model, window, &mut self.eeg_view_state),
            }
        }

        //         // LEFT BUTTON
        //         let left_color = self.left_button_color;
        //         self.sound_click.execute(|_| {
        //             window.draw(&RECT_LEFT_BUTTON, Col(left_color));
        //             Ok(())
        //         })?;
        //         self.left_button_color = COLOR_BUTTON;

        //         // RIGHT BUTTON
        //         let right_color = self.right_button_color;
        //         self.sound_click.execute(|_| {
        //             window.draw(&RECT_RIGHT_BUTTON, Col(right_color));
        //             Ok(())
        //         })?;
        //         self.right_button_color = COLOR_BUTTON;
        //     } else {
        //         // LOGO
        //         self.logo.execute(|image| {
        //             window.draw(
        //                 &image
        //                     .area()
        //                     .with_center((SCREEN_SIZE.0 / 2.0, SCREEN_SIZE.1 / 2.0)),
        //                 Img(&image),
        //             );
        //             Ok(())
        //         })?;
        //     }

        self.frame_count = self.frame_count + 1;
        if self.frame_count == std::u64::MAX {
            self.frame_count = 1;
        }

        Ok(())
    }

    fn handle_error(error: quicksilver::Error) {
        error!("Unhandled error: {:?}", error);
        panic!("Unhandled error: {:?}", error);
    }
}

fn main() {
    use quicksilver::graphics::*;

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        env_logger::init();
    }

    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        web_logger::init();
    }

    info!("meme_quicksilver start");
    let draw_rate: f64 = 1000. / FPS as f64;
    let update_rate: f64 = 1000. / UPS as f64;

    let settings = Settings {
        icon_path: Some("n-icon.png"),
        fullscreen: true,
        resize: ResizeStrategy::Fit,
        draw_rate,
        update_rate,
        ..Settings::default()
    };

    run::<AppState>(
        STR_TITLE,
        Vector::new(SCREEN_SIZE.0, SCREEN_SIZE.1),
        settings,
    )
}
