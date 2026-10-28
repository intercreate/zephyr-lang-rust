// Copyright The Zephyr Project Contributors
// SPDX-License-Identifier: Apache-2.0

#![no_std]

use core::{ffi::c_int, future::Future};

use embassy_futures::{
    join::join,
    select::{select, Either},
};
use embassy_time::Timer;
use embedded_hal_async::digital::Wait;
use static_cell::StaticCell;
use zephyr::{device::gpio::GpioPin, embassy::Executor, printkln, raw, Error};
use zephyr_panic as _;

/// The `gpio0` pin that `wait_pin` uses in the board overlay.
const WAIT_PIN: raw::gpio_pin_t = 1;

/// Long enough for an emulated interrupt to arrive, short enough to keep the test fast.
const TIMEOUT_MS: u64 = 20;

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[no_mangle]
extern "C" fn rust_main() {
    EXECUTOR.init(Executor::new()).run(|spawner| {
        spawner.spawn(main().unwrap());
    })
}

#[derive(Clone, Copy)]
enum Expect {
    Completes,
    TimesOut,
}

/// The pin levels a case drives before and after its wait starts.
#[derive(Clone, Copy)]
enum Transition {
    Rise,
    Fall,
    StayHigh,
}

impl Transition {
    const fn levels(self) -> (bool, bool) {
        match self {
            Self::Rise => (false, true),
            Self::Fall => (true, false),
            Self::StayHigh => (true, true),
        }
    }
}

#[embassy_executor::task]
async fn main() {
    use Expect::{Completes, TimesOut};
    use Transition::{Fall, Rise, StayHigh};

    let mut pin = zephyr::devicetree::labels::wait_pin::get_instance().unwrap();
    pin.configure(raw::ZR_GPIO_INPUT);

    let passed = [
        case(
            "wait_for_high",
            Wait::wait_for_high(&mut pin),
            Rise,
            Completes,
        )
        .await,
        case(
            "wait_for_low",
            Wait::wait_for_low(&mut pin),
            Fall,
            Completes,
        )
        .await,
        case(
            "wait_for_rising_edge",
            Wait::wait_for_rising_edge(&mut pin),
            Rise,
            Completes,
        )
        .await,
        case(
            "wait_for_falling_edge",
            Wait::wait_for_falling_edge(&mut pin),
            Fall,
            Completes,
        )
        .await,
        case(
            "wait_for_any_edge",
            Wait::wait_for_any_edge(&mut pin),
            Rise,
            Completes,
        )
        .await,
        case(
            "rising_edge_ignores_high",
            Wait::wait_for_rising_edge(&mut pin),
            StayHigh,
            TimesOut,
        )
        .await,
        cancelled_wait_leaves_no_event(&mut pin).await,
    ];

    let verdict = if passed.iter().all(|&ok| ok) {
        "all passed"
    } else {
        "FAILED"
    };
    printkln!("gpio-wait: {verdict}");
}

/// Drive the transition's first level, start `wait` (its first poll arms the interrupt), drive the
/// second level, and check the outcome against `expect`.
async fn case(
    name: &str,
    wait: impl Future<Output = Result<(), Error>>,
    transition: Transition,
    expect: Expect,
) -> bool {
    let (before, after) = transition.levels();
    drive(before);
    let driven = async {
        Timer::after_millis(1).await;
        drive(after);
    };
    let outcome = within(join(wait, driven)).await.map(|(waited, ())| waited);
    report(
        name,
        matches!(
            (outcome, expect),
            (Some(Ok(())), Expect::Completes) | (None, Expect::TimesOut)
        ),
    )
}

/// An edge that arrives after its wait was cancelled must not complete the next wait.
async fn cancelled_wait_leaves_no_event(pin: &mut GpioPin) -> bool {
    drive(false);
    let cancelled = within(Wait::wait_for_rising_edge(pin)).await.is_none();
    drive(true);
    let stale = within(Wait::wait_for_rising_edge(pin)).await.is_some();
    report("cancelled_wait_leaves_no_event", cancelled && !stale)
}

async fn within<F: Future>(future: F) -> Option<F::Output> {
    match select(future, Timer::after_millis(TIMEOUT_MS)).await {
        Either::First(output) => Some(output),
        Either::Second(()) => None,
    }
}

fn report(name: &str, passed: bool) -> bool {
    let verdict = if passed { "PASS" } else { "FAIL" };
    printkln!("{verdict} {name}");
    passed
}

fn drive(level: bool) {
    // SAFETY: `gpio0` is native_sim's GPIO emulator, a `'static` device that this test only drives
    // through the emulator API, never through a second Rust `Gpio` wrapper.
    let status = unsafe {
        raw::gpio_emul_input_set(
            zephyr::devicetree::labels::gpio0::get_instance_raw(),
            WAIT_PIN,
            c_int::from(level),
        )
    };
    assert_eq!(status, 0, "gpio_emul_input_set failed");
}
