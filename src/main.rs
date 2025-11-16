use std::{
    ffi::CString,
    sync::{
        atomic::{AtomicBool, Ordering},
        RwLock,
    },
    time::Instant,
};

use anyhow::Result;
use embedded_graphics::{draw_target::DrawTarget, pixelcolor::Rgb565, prelude::Point};
use esp_idf_svc::hal::{
    delay::{Delay, FreeRtos},
    gpio::PinDriver,
    prelude::Peripherals,
    spi::{
        config::{Config, DriverConfig},
        Dma, SpiDeviceDriver,
    },
    units::MegaHertz,
};
use log::info;
use lv_bevy_ecs::{
    display::{Display, DrawBuffer},
    events::Event,
    functions::*,
    input::{BufferStatus, InputDevice, InputEvent, InputState, Pointer},
    support::{Align, LabelLongMode},
    widgets::{Arc, Label},
};
use mipidsi::{interface::SpiInterface, models::ST7789, Builder};
use xpt2046::{TouchEvent, TouchKind, TouchScreen, Xpt2046};

static IS_POINTER_DOWN: AtomicBool = AtomicBool::new(false);

static LATEST_TOUCH_STATUS: RwLock<InputEvent<Pointer>> =
    RwLock::new(InputEvent::default_const(Point::zero()));

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    // Forward LVGL logs to EspLogger
    lv_bevy_ecs::logging::connect();

    // Use LVGL logger instead
    //lv_log_init();

    const HOR_RES: u32 = 320;
    const VER_RES: u32 = 240;
    const LINE_HEIGHT: u32 = VER_RES / 20;

    let mut delay: Delay = Default::default();

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;

    let mut buffer_ref = [0u8; 320]; //SCREEN_BUFFER.init([0u8; 320]);
    let di = SpiInterface::new(
        SpiDeviceDriver::new_single(
            peripherals.spi2,
            pins.gpio14,
            pins.gpio13,
            Some(pins.gpio12),
            Some(pins.gpio15),
            &DriverConfig::default().dma(Dma::Auto(buffer_ref.len())),
            &Config::default().baudrate(MegaHertz(40).into()),
        )?,
        PinDriver::output(pins.gpio2)?,
        &mut buffer_ref,
    );

    let mut tft_display = Builder::new(ST7789, di)
        .color_order(mipidsi::options::ColorOrder::Rgb)
        .orientation(
            mipidsi::options::Orientation::default().rotate(mipidsi::options::Rotation::Deg270),
        ) // Mirror on text
        .reset_pin(PinDriver::output(pins.gpio4)?)
        .init(&mut delay)
        .expect("Could not initialize display");

    let touch_clk = pins.gpio25;
    let touch_mosi = pins.gpio32;
    let touch_cs = pins.gpio33;
    let touch_miso = pins.gpio39;

    let touch_driver = SpiDeviceDriver::new_single(
        peripherals.spi3,
        touch_clk,
        touch_mosi,
        Some(touch_miso),
        Some(touch_cs),
        &DriverConfig::new(),
        &Config::new(), //.baudrate(MegaHertz(2).into()).queue_size(3),
    )?;

    let mut touch = Xpt2046::new(touch_driver, None);

    //===========================================================================================================
    //                               Create the User Interface
    //===========================================================================================================

    // Pin 21, Backlight
    let mut bl = PinDriver::output(pins.gpio21)?;
    // Turn on backlight
    bl.set_high()?;

    if !touch.calibrated() {
        // Display is uncalibrated, resolve that before we do anything else.
        let output = touch
            .intrusive_calibration(&mut tft_display, &mut delay)
            .expect("Could not calibrate");
        dbg!(&output);
    }

    let mut display = Display::create(HOR_RES as i32, VER_RES as i32);
    let buffer =
        DrawBuffer::<{ (HOR_RES * LINE_HEIGHT) as usize }, Rgb565>::create(HOR_RES, LINE_HEIGHT);
    info!("Display OK");
    display.register(buffer, |refresh| {
        let area = refresh.rectangle;
        let data = refresh.colors.iter().cloned();

        tft_display
            .fill_contiguous(&area, data)
            .expect("Cannot fill display");
    });

    info!("Draw Buffer OK");

    //let mut world = LvglWorld::default();
    //world.add_observer(on_insert_children);

    //info!("World OK");

    // Create screen and widgets
    //let mut screen: lvgl::Screen = display.get_scr_act().map_err(BoardError::DISPLAY)?;

    // let mut screen_style = Style::default();
    // screen_style.set_bg_color(Color::from_rgb((100, 100, 100)));
    // screen.add_style(Part::Main, &mut screen_style);

    let mut arc = Arc::create_widget();
    lv_obj_set_size(&mut arc, 150, 150);
    lv_arc_set_rotation(&mut arc, 135);
    lv_arc_set_bg_angles(&mut arc, 0, 270);
    lv_arc_set_value(&mut arc, 10);
    lv_obj_set_align(&mut arc, Align::Center.into());

    let mut label = Label::create_widget();
    lv_label_set_long_mode(&mut label, LabelLongMode::Clip.into());
    lv_label_set_text_static(&mut label, c"asdasdasd");
    lv_obj_set_align(&mut label, Align::TopMid.into());

    lv_obj_add_event_cb(&mut arc, Event::ValueChanged, |mut event| {
        let Some(mut obj) = lv_event_get_target_obj(&mut event) else {
            lv_bevy_ecs::warn!("Target obj was null");
            return;
        };
        let value = lv_arc_get_value(&mut obj);
        let text = CString::new(value.to_string()).unwrap();
        lv_label_set_text(&mut label, text.as_c_str());
    });

    /*world.spawn(label);
    world.spawn(arc);*/

    info!("Widgets OK");

    let _pointer = InputDevice::<Pointer>::create(|| {
        match touch.get_touch_event() {
            Ok(event) => event.iter().for_each(update_touch_input),
            Err(error) => {
                dbg!(error);
            }
        }
        let lock = LATEST_TOUCH_STATUS.read().unwrap();
        return *lock;
    });

    info!("Pointer OK");

    let mut prev_time = Instant::now();

    FreeRtos::delay_ms(10);
    info!("Sleep OK");

    loop {
        let current_time = Instant::now();
        let diff = current_time.duration_since(prev_time);
        prev_time = current_time;

        lv_tick_inc(diff);
        lv_timer_handler();

        FreeRtos::delay_ms(10);
    }
}

fn update_touch_input(event: &TouchEvent) {
    let mut next_touch_status = None;

    match event.kind {
        TouchKind::Start => {
            next_touch_status = Some(InputEvent {
                status: BufferStatus::Once,
                state: InputState::Pressed,
                data: event.point,
            });
            IS_POINTER_DOWN.store(true, Ordering::Release);
        }
        TouchKind::Move => {
            if IS_POINTER_DOWN.load(Ordering::Acquire) {
                next_touch_status = Some(InputEvent {
                    status: BufferStatus::Once,
                    state: InputState::Pressed,
                    data: event.point,
                });
            }
        }
        TouchKind::End => {
            next_touch_status = Some(InputEvent {
                status: BufferStatus::Once,
                state: InputState::Released,
                data: Point::new(0, 0),
            });
            IS_POINTER_DOWN.store(false, Ordering::Release);
        }
    }
    if let Some(latest_touch_status) = next_touch_status {
        let mut lock = LATEST_TOUCH_STATUS.write().unwrap();
        *lock = latest_touch_status;
    }
}
