use embedded_hal::digital::{ErrorKind, ErrorType};
use embedded_hal_async::digital::Wait;

use crate::device::gpio::{GpioPin, GpioWait, Trigger};
use crate::error::{Error, Result};

impl embedded_hal::digital::Error for Error {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

impl ErrorType for GpioPin {
    type Error = Error;
}

impl Wait for GpioPin {
    async fn wait_for_high(&mut self) -> Result<()> {
        GpioWait::new(self, Trigger::LevelHigh).await
    }

    async fn wait_for_low(&mut self) -> Result<()> {
        GpioWait::new(self, Trigger::LevelLow).await
    }

    async fn wait_for_rising_edge(&mut self) -> Result<()> {
        GpioWait::new(self, Trigger::EdgeRising).await
    }

    async fn wait_for_falling_edge(&mut self) -> Result<()> {
        GpioWait::new(self, Trigger::EdgeFalling).await
    }

    async fn wait_for_any_edge(&mut self) -> Result<()> {
        GpioWait::new(self, Trigger::EdgeBoth).await
    }
}
