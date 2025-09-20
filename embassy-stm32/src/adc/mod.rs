//! Analog to Digital Converter (ADC)

#![macro_use]
#![allow(missing_docs)] // TODO
#![cfg_attr(adc_f3v3, allow(unused))]

#[cfg(not(any(adc_f3v3, adc_wba)))]
#[cfg_attr(adc_f1, path = "f1.rs")]
#[cfg_attr(adc_f3v1, path = "f3.rs")]
#[cfg_attr(adc_f3v2, path = "f3_v1_1.rs")]
#[cfg_attr(adc_v1, path = "v1.rs")]
#[cfg_attr(adc_l0, path = "v1.rs")]
#[cfg_attr(adc_v2, path = "v2.rs")]
#[cfg_attr(any(adc_v3, adc_g0, adc_h5, adc_h7rs, adc_u0), path = "v3.rs")]
#[cfg_attr(any(adc_v4, adc_u5), path = "v4.rs")]
#[cfg_attr(adc_g4, path = "g4.rs")]
#[cfg_attr(adc_c0, path = "c0.rs")]
mod _version;

use core::{
    future::{poll_fn, Future, IntoFuture},
    marker::PhantomData,
    pin::Pin,
    task::Poll,
    usize,
};

#[allow(unused)]
#[cfg(not(any(adc_f3v3, adc_wba)))]
pub use _version::*;
use embassy_hal_internal::{impl_peripheral, Peri, PeripheralType};
#[cfg(any(adc_f1, adc_f3v1, adc_v1, adc_l0, adc_f3v2))]
use embassy_sync::waitqueue::AtomicWaker;
use futures_util::{Stream, StreamExt};

#[cfg(any(adc_u5, adc_wba))]
#[path = "adc4.rs"]
pub mod adc4;

pub use crate::pac::adc::vals;
#[cfg(not(any(adc_f1, adc_f3v3)))]
pub use crate::pac::adc::vals::Res as Resolution;
pub use crate::pac::adc::vals::SampleTime;
use crate::{
    dma::{AnyChannel, ChannelState, DmaCtrlImpl, Priority, Request, Transfer, TransferEvent, TransferOptions},
    peripherals,
};

#[cfg(not(adc_wba))]
dma_trait!(RxDma, Instance);
#[cfg(adc_u5)]
dma_trait!(RxDma4, adc4::Instance);
#[cfg(adc_wba)]
dma_trait!(RxDma4, adc4::Instance);

pub struct Buffer<'a, const N: usize> {
    buffer: [u16; N],
    transfer: Transfer<'a>,
    // transfer: Pin<Transfer<'a>>,
}

pub struct NoBuffer;

pub trait Buffered {
    const FOO: usize;
}
impl<'a, const N: usize> Buffered for Buffer<'a, N> {
    const FOO: usize = N;
}
impl Buffered for NoBuffer {
    const FOO: usize = 0;
}

/// Analog to Digital driver.
pub struct Adc<'d, T: Instance, B: Buffered = NoBuffer> {
    #[allow(unused)]
    adc: crate::Peri<'d, T>,
    #[cfg(not(any(adc_f3v3, adc_f3v2, adc_wba)))]
    sample_time: SampleTime,
    buffer: B,
}

// struct Foo<'a>([u8; 1], &'a mut [u8]);
// fn foo<'a>() -> Foo<'a> {
//     let mut x = [0];
//     Foo(x, &mut x)
// }

impl<'d, T: Instance> Adc<'d, T> {
    pub fn into_buffered<'a, const N: usize>(
        self,
        dma: Peri<'a, impl RxDma<T>>,
        dma_prio: Priority,
        mut buffer: [u16; N],
    ) -> Adc<'d, T, Buffer<'a, N>> {
        let options = TransferOptions {
            circular: true,
            half_transfer_ir: true,
            complete_transfer_ir: true,
            priority: dma_prio,
        };
        let request = dma.request();
        let transfer: Transfer<'a> =
            unsafe { Transfer::new_read_raw(dma, request, T::regs().dr().as_ptr() as *mut u16, &mut buffer, options) };
        // let x: dyn Future<Output=()>=  transfer.into();
        // let x = transfer.into_future();
        // x.po
        // transfer.await;
        Adc {
            adc: self.adc,
            sample_time: self.sample_time,
            buffer: Buffer {
                buffer,
                transfer: transfer.into(),
                // transfer: Pin::<_> { transfer },
            },
        }
    }
}

impl<'d, 'a, T: Instance, const N: usize> Adc<'d, T, Buffer<'a, N>> {
    pub fn read(&self) -> impl Stream<Item = &[u16]> {
        self.buffer.transfer.completions().map(|ev| match ev {
            TransferEvent::Half => &self.buffer.buffer[..N / 2],
            TransferEvent::Complete => &self.buffer.buffer[N / 2..],
        })
    }

    // pub async fn read<'b: 'd + 'a>(&self) -> &'b [u16] {
    //     // let foo = Pin<&mut Self>{&mut self};
    //     poll_fn(|cx| {
    //         let state: &ChannelState = &STATE[self.channel.id as usize];

    //         state.waker.register(cx.waker());
    //         Poll::Ready(&self.buffer.buffer[..N / 2])
    //     })
    //     .await
    // }
    // pub async fn read_exact(&mut self, buffer: &mut [W]) -> Result<usize, Error> {
    //     let mut read_data = 0;
    //     let buffer_len = buffer.len();
    //     let dma = &mut DmaCtrlImpl(self.channel.reborrow())

    //     poll_fn(|cx| {
    //         dma.set_waker(cx.waker());

    //         match self.read(dma, &mut buffer[read_data..buffer_len]) {
    //             Ok((len, remaining)) => {
    //                 read_data += len;
    //                 if read_data == buffer_len {
    //                     Poll::Ready(Ok(remaining))
    //                 } else {
    //                     Poll::Pending
    //                 }
    //             }
    //             Err(e) => Poll::Ready(Err(e)),
    //         }
    //     })
    //     .await
    // }
}

#[cfg(any(adc_f1, adc_f3v1, adc_v1, adc_l0, adc_f3v2))]
pub struct State {
    pub waker: AtomicWaker,
}

#[cfg(any(adc_f1, adc_f3v1, adc_v1, adc_l0, adc_f3v2))]
impl State {
    pub const fn new() -> Self {
        Self {
            waker: AtomicWaker::new(),
        }
    }
}

trait SealedInstance {
    #[cfg(not(adc_wba))]
    #[allow(unused)]
    fn regs() -> crate::pac::adc::Adc;
    #[cfg(not(any(adc_f1, adc_v1, adc_l0, adc_f3v3, adc_f3v2, adc_g0)))]
    #[allow(unused)]
    fn common_regs() -> crate::pac::adccommon::AdcCommon;
    #[cfg(any(adc_f1, adc_f3v1, adc_v1, adc_l0, adc_f3v2))]
    fn state() -> &'static State;
}

pub(crate) trait SealedAdcChannel<T> {
    #[cfg(any(adc_v1, adc_c0, adc_l0, adc_v2, adc_g4, adc_v4, adc_u5, adc_wba))]
    fn setup(&mut self) {}

    #[allow(unused)]
    fn channel(&self) -> u8;
}

/// Performs a busy-wait delay for a specified number of microseconds.
#[allow(unused)]
pub(crate) fn blocking_delay_us(us: u32) {
    #[cfg(feature = "time")]
    embassy_time::block_for(embassy_time::Duration::from_micros(us as u64));
    #[cfg(not(feature = "time"))]
    {
        let freq = unsafe { crate::rcc::get_freqs() }.sys.to_hertz().unwrap().0 as u64;
        let us = us as u64;
        let cycles = freq * us / 1_000_000;
        cortex_m::asm::delay(cycles as u32);
    }
}

/// ADC instance.
#[cfg(not(any(
    adc_f1, adc_v1, adc_l0, adc_v2, adc_v3, adc_v4, adc_g4, adc_f3v1, adc_f3v2, adc_g0, adc_u0, adc_h5, adc_h7rs,
    adc_u5, adc_c0, adc_wba,
)))]
#[allow(private_bounds)]
pub trait Instance: SealedInstance + crate::PeripheralType {
    type Interrupt: crate::interrupt::typelevel::Interrupt;
}
/// ADC instance.
#[cfg(any(
    adc_f1, adc_v1, adc_l0, adc_v2, adc_v3, adc_v4, adc_g4, adc_f3v1, adc_f3v2, adc_g0, adc_u0, adc_h5, adc_h7rs,
    adc_u5, adc_c0, adc_wba,
))]
#[allow(private_bounds)]
pub trait Instance: SealedInstance + crate::PeripheralType + crate::rcc::RccPeripheral {
    type Interrupt: crate::interrupt::typelevel::Interrupt;
}

/// ADC channel.
#[allow(private_bounds)]
pub trait AdcChannel<T>: SealedAdcChannel<T> + Sized {
    #[allow(unused_mut)]
    fn degrade_adc(mut self) -> AnyAdcChannel<T> {
        #[cfg(any(adc_v1, adc_l0, adc_v2, adc_g4, adc_v4, adc_u5, adc_wba))]
        self.setup();

        AnyAdcChannel {
            channel: self.channel(),
            _phantom: PhantomData,
        }
    }
}

/// A type-erased channel for a given ADC instance.
///
/// This is useful in scenarios where you need the ADC channels to have the same type, such as
/// storing them in an array.
pub struct AnyAdcChannel<T> {
    channel: u8,
    _phantom: PhantomData<T>,
}
impl_peripheral!(AnyAdcChannel<T: Instance>);
impl<T: Instance> AdcChannel<T> for AnyAdcChannel<T> {}
impl<T: Instance> SealedAdcChannel<T> for AnyAdcChannel<T> {
    fn channel(&self) -> u8 {
        self.channel
    }
}

impl<T> AnyAdcChannel<T> {
    #[allow(unused)]
    pub fn get_hw_channel(&self) -> u8 {
        self.channel
    }
}
#[cfg(adc_wba)]
foreach_adc!(
    (ADC4, $common_inst:ident, $clock:ident) => {
        impl crate::adc::adc4::SealedInstance for peripherals::ADC4 {
            fn regs() -> crate::pac::adc::Adc4 {
                crate::pac::ADC4
            }
        }

        impl crate::adc::adc4::Instance for peripherals::ADC4 {
            type Interrupt = crate::_generated::peripheral_interrupts::ADC4::GLOBAL;
        }
    };

    ($inst:ident, $common_inst:ident, $clock:ident) => {
        impl crate::adc::SealedInstance for peripherals::$inst {
            fn regs() -> crate::pac::adc::Adc {
                crate::pac::$inst
            }

            fn common_regs() -> crate::pac::adccommon::AdcCommon {
                return crate::pac::$common_inst
            }
        }

        impl crate::adc::Instance for peripherals::$inst {
            type Interrupt = crate::_generated::peripheral_interrupts::$inst::GLOBAL;
        }
    };
);

#[cfg(adc_u5)]
foreach_adc!(
    (ADC4, $common_inst:ident, $clock:ident) => {
        impl crate::adc::adc4::SealedInstance for peripherals::ADC4 {
            fn regs() -> crate::pac::adc::Adc4 {
                crate::pac::ADC4
            }
        }

        impl crate::adc::adc4::Instance for peripherals::ADC4 {
            type Interrupt = crate::_generated::peripheral_interrupts::ADC4::GLOBAL;
        }
    };

    ($inst:ident, $common_inst:ident, $clock:ident) => {
        impl crate::adc::SealedInstance for peripherals::$inst {
            fn regs() -> crate::pac::adc::Adc {
                crate::pac::$inst
            }

            fn common_regs() -> crate::pac::adccommon::AdcCommon {
                return crate::pac::$common_inst
            }
        }

        impl crate::adc::Instance for peripherals::$inst {
            type Interrupt = crate::_generated::peripheral_interrupts::$inst::GLOBAL;
        }
    };
);

#[cfg(not(any(adc_u5, adc_wba)))]
foreach_adc!(
    ($inst:ident, $common_inst:ident, $clock:ident) => {
        impl crate::adc::SealedInstance for peripherals::$inst {
            #[cfg(not(adc_wba))]
            fn regs() -> crate::pac::adc::Adc {
                crate::pac::$inst
            }

            #[cfg(adc_wba)]
            fn regs() -> crate::pac::adc::Adc4 {
                crate::pac::$inst
            }

            #[cfg(not(any(adc_f1, adc_v1, adc_l0, adc_f3v3, adc_f3v2, adc_g0, adc_u5, adc_wba)))]
            fn common_regs() -> crate::pac::adccommon::AdcCommon {
                return crate::pac::$common_inst
            }

            #[cfg(any(adc_f1, adc_f3v1, adc_v1, adc_l0, adc_f3v2))]
            fn state() -> &'static State {
                static STATE: State = State::new();
                &STATE
            }
        }

        impl crate::adc::Instance for peripherals::$inst {
            type Interrupt = crate::_generated::peripheral_interrupts::$inst::GLOBAL;
        }
    };
);

macro_rules! impl_adc_pin {
    ($inst:ident, $pin:ident, $ch:expr) => {
        impl crate::adc::AdcChannel<peripherals::$inst> for crate::Peri<'_, crate::peripherals::$pin> {}
        impl crate::adc::SealedAdcChannel<peripherals::$inst> for crate::Peri<'_, crate::peripherals::$pin> {
            #[cfg(any(adc_v1, adc_c0, adc_l0, adc_v2, adc_g4, adc_v4, adc_u5, adc_wba))]
            fn setup(&mut self) {
                <crate::peripherals::$pin as crate::gpio::SealedPin>::set_as_analog(self);
            }

            fn channel(&self) -> u8 {
                $ch
            }
        }
    };
}

/// Get the maximum reading value for this resolution.
///
/// This is `2**n - 1`.
#[cfg(not(any(adc_f1, adc_f3v3)))]
pub const fn resolution_to_max_count(res: Resolution) -> u32 {
    match res {
        #[cfg(adc_v4)]
        Resolution::BITS16 => (1 << 16) - 1,
        #[cfg(any(adc_v4, adc_u5))]
        Resolution::BITS14 => (1 << 14) - 1,
        #[cfg(adc_v4)]
        Resolution::BITS14V => (1 << 14) - 1,
        #[cfg(adc_v4)]
        Resolution::BITS12V => (1 << 12) - 1,
        Resolution::BITS12 => (1 << 12) - 1,
        Resolution::BITS10 => (1 << 10) - 1,
        Resolution::BITS8 => (1 << 8) - 1,
        #[cfg(any(adc_v1, adc_v2, adc_v3, adc_l0, adc_c0, adc_g0, adc_f3v1, adc_f3v2, adc_h5))]
        Resolution::BITS6 => (1 << 6) - 1,
        #[allow(unreachable_patterns)]
        _ => core::unreachable!(),
    }
}
