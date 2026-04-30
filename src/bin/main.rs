#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use alloc::{ffi::CString, string::ToString};
use defmt_serial as _;
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{DrawTarget, Point};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::spi::master::Spi;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::uart::{Config, Uart};
use esp_hal::{Blocking, spi};
use lv_bevy_ecs::display::{Display, DrawBuffer};
use lv_bevy_ecs::events::EventCode;
use lv_bevy_ecs::functions::{NextTimerPeriod, lv_tick_set_cb, lv_timer_handler};
use lv_bevy_ecs::input::{BufferStatus, InputDevice, InputEvent, InputState, Pointer};
use lv_bevy_ecs::support::{Align, LabelLongMode};
use lv_bevy_ecs::widgets::{Arc, Label, Wdg};
use lvgl_bevy_demo_nostd::heap::get_memory_stats;
use mipidsi::Builder;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ST7789;
use static_cell::StaticCell;
use xpt2046::{CalibrationData, TouchEvent, TouchKind, TouchScreen, Xpt2046};

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static SERIAL: StaticCell<Uart<'static, Blocking>> = StaticCell::new();

// #[panic_handler]
// pub fn panic(info: &::core::panic::PanicInfo) -> ! {
//     defmt::error!("{}", info);
//     loop {}
// }

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    // generator version: 1.2.0

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let uart = Uart::new(peripherals.UART0, Config::default())
        .unwrap()
        .with_rx(peripherals.GPIO3)
        .with_tx(peripherals.GPIO1);

    let serial = SERIAL.init(uart);

    defmt_serial::defmt_serial(serial);

    lvgl_bevy_demo_nostd::heap::setup_heap();

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let swint = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, swint.software_interrupt0);

    defmt::info!("Embassy initialized!");

    lv_bevy_ecs::functions::lv_init();
    lv_bevy_ecs::logging::connect();
    lv_bevy_ecs::malloc::provide_mem_monitor_impl(get_memory_stats);

    const HOR_RES: u32 = 320;
    const VER_RES: u32 = 240;
    const BUF_HEIGHT: u32 = VER_RES / 20;

    let mut delay: Delay = Default::default();

    let mut buffer_ref = [0u8; 512]; //SCREEN_BUFFER.init([0u8; 320]);
    let di = SpiInterface::new(
        ExclusiveDevice::new(
            Spi::new(
                peripherals.SPI2,
                spi::master::Config::default().with_frequency(Rate::from_mhz(20)),
            )
            .unwrap()
            .with_mosi(peripherals.GPIO13)
            .with_miso(peripherals.GPIO12)
            .with_sck(peripherals.GPIO14),
            Output::new(peripherals.GPIO15, Level::High, OutputConfig::default()),
            &mut delay,
        )
        .unwrap(),
        Output::new(peripherals.GPIO2, Level::High, OutputConfig::default()),
        &mut buffer_ref,
    );

    let mut tft_display = Builder::new(ST7789, di)
        .color_order(mipidsi::options::ColorOrder::Rgb)
        .orientation(
            mipidsi::options::Orientation::default().rotate(mipidsi::options::Rotation::Deg270),
        ) // Mirror on text
        .reset_pin(Output::new(
            peripherals.GPIO4,
            Level::High,
            OutputConfig::default(),
        ))
        .init(&mut Delay::default())
        .expect("Could not initialize display");

    let touch_driver = ExclusiveDevice::new(
        Spi::new(
            peripherals.SPI3,
            spi::master::Config::default().with_frequency(Rate::from_mhz(1)),
        )
        .unwrap()
        .with_mosi(peripherals.GPIO32)
        .with_miso(peripherals.GPIO39)
        .with_sck(peripherals.GPIO25),
        Output::new(peripherals.GPIO33, Level::High, OutputConfig::default()),
        Delay::default(),
    )
    .unwrap();

    let calibration_data = CalibrationData {
        alpha_x: -0.09,
        beta_x: 0.001,
        delta_x: 345.0,
        alpha_y: 0.0008,
        beta_y: -0.07,
        delta_y: 250.0,
    };

    let mut touch = Xpt2046::new(touch_driver, Some(calibration_data));

    //===========================================================================================================
    //                               Create the User Interface
    //===========================================================================================================

    Output::new(peripherals.GPIO21, Level::High, OutputConfig::default());

    // if !touch.calibrated() {
    //     // Display is uncalibrated, resolve that before we do anything else.
    //     let output = touch
    //         .intrusive_calibration(&mut tft_display, &mut Delay::default())
    //         .expect("Could not calibrate");
    //     defmt::debug!("{}", DebugCalibrationData(output));
    // }

    let mut display = Display::new(HOR_RES as i32, VER_RES as i32);
    let buffer =
        DrawBuffer::<{ (HOR_RES * BUF_HEIGHT) as usize }, Rgb565>::new(HOR_RES, BUF_HEIGHT);
    defmt::info!("Display OK");
    display.register(buffer, |refresh| {
        let area = refresh.rectangle;
        let data = refresh.colors.iter().cloned();

        tft_display
            .fill_contiguous(&area, data)
            .expect("Cannot fill display");

        refresh.display.flush_ready();
    });

    defmt::info!("Draw Buffer OK");

    let mut arc = Arc::new();
    arc.set_size(150, 150);
    arc.set_rotation(135);
    arc.set_bg_angles(0, 270);
    arc.set_value(10);
    arc.set_align(Align::Center.into());

    let mut label = Label::new();
    label.set_long_mode(LabelLongMode::Clip.into());
    label.set_text_static(c"asdasdasd");
    label.set_align(Align::TopMid.into());

    arc.add_event_cb(EventCode::ValueChanged, |mut event| {
        let Some(obj) = event.get_target_obj() else {
            defmt::warn!("Target obj was null");
            return;
        };
        let value = obj.downcast::<Arc<Wdg>>().unwrap().get_value();
        let text = CString::new(value.to_string()).unwrap();
        label.set_text(text.as_c_str());
    });

    defmt::info!("Widgets OK");

    let _pointer = InputDevice::<Pointer>::new(|| {
        let event = touch.get_touch_event();
        if let Err(_error) = event {
            defmt::error!("Error reading touch event");
        }
        get_touch_input(event.ok().flatten())
    });

    defmt::info!("Pointer OK");

    lv_tick_set_cb(|| {
        let now = Instant::now();
        now.as_millis() as u32
    });

    loop {
        let frame_start = Instant::now();
        let delay = lv_timer_handler();
        match delay {
            NextTimerPeriod::Ready => {
                continue;
            }
            NextTimerPeriod::AfterMs(delay) => {
                //esp_idf_svc::sys::xTaskDelayUntil(&mut tick, delay.get());
                Timer::at(frame_start + Duration::from_millis(delay.get().into())).await;
            }
            NextTimerPeriod::Never => {
                Timer::after_secs(1).await;
                //esp_idf_svc::sys::vTaskDelay(LV_DEF_REFR_PERIOD);
            }
        }
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0/examples
}

fn get_touch_input(event: Option<TouchEvent>) -> InputEvent<Pointer> {
    // static IS_POINTER_DOWN: AtomicBool = AtomicBool::new(false);
    // static LATEST_TOUCH_STATUS: Mutex<InputEvent<Pointer>> =
    //     Mutex::new(InputEvent::new(Point::zero()));
    static mut IS_POINTER_DOWN: bool = false;
    static mut LATEST_TOUCH_STATUS: InputEvent<Pointer> = InputEvent::new(Point::zero());

    unsafe {
        let Some(event) = event else {
            return LATEST_TOUCH_STATUS;
        };

        let mut next_touch_status = None;

        match event.kind {
            TouchKind::Start => {
                next_touch_status = Some(InputEvent {
                    status: BufferStatus::Once,
                    state: InputState::Pressed,
                    data: event.point,
                });
                IS_POINTER_DOWN = true;
            }
            TouchKind::Move => {
                if IS_POINTER_DOWN {
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
                IS_POINTER_DOWN = false;
            }
        }

        if let Some(latest_touch_status) = next_touch_status {
            LATEST_TOUCH_STATUS = latest_touch_status;
        }
        return LATEST_TOUCH_STATUS;
    }
}

#[allow(unused)]
struct DebugCalibrationData(CalibrationData);

impl defmt::Format for DebugCalibrationData {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "CalibrationData {{
        alpha_x:{},
        beta_x: {},
        delta_x: {},
        alpha_y: {},
        beta_y: {},
        delta_y: {},
    }}",
            self.0.alpha_x,
            self.0.beta_x,
            self.0.delta_x,
            self.0.alpha_y,
            self.0.beta_y,
            self.0.delta_y
        );
    }
}
