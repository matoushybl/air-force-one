use embassy_nrf::{interrupt, peripherals, qspi};

pub const EXTERNAL_FLASH_SIZE: usize = 2097152;
pub const EXTERNAL_FLASH_BLOCK_SIZE: usize = 256;
pub type ExternalFlash<'d> = qspi::Qspi<'d, peripherals::QSPI, EXTERNAL_FLASH_SIZE>;

/// Pins for External QSPI flash
pub struct ExternalFlashPins {
    pub qspi: peripherals::QSPI,
    pub sck: peripherals::P0_08,
    pub csn: peripherals::P0_04,
    pub io0: peripherals::P0_06,
    pub io1: peripherals::P0_26,
    pub io2: peripherals::P0_27,
    pub io3: peripherals::P1_09,
}

impl ExternalFlashPins {
    /// Configure an external flash instance based on pins
    pub fn configure<'d>(self, irq: interrupt::QSPI) -> ExternalFlash<'d> {
        let mut config = qspi::Config::default();
        config.read_opcode = qspi::ReadOpcode::READ4IO;
        config.write_opcode = qspi::WriteOpcode::PP4O;
        config.write_page_size = qspi::WritePageSize::_256BYTES;
        let mut q: qspi::Qspi<'_, _, EXTERNAL_FLASH_SIZE> = qspi::Qspi::new(
            self.qspi, irq, self.sck, self.csn, self.io0, self.io1, self.io2, self.io3, config,
        );

        // Setup QSPI
        let mut status = [4; 2];
        q.blocking_custom_instruction(0x05, &[], &mut status[..1])
            .unwrap();

        q.blocking_custom_instruction(0x35, &[], &mut status[1..2])
            .unwrap();

        if status[1] & 0x02 == 0 {
            status[1] |= 0x02;
            q.blocking_custom_instruction(0x01, &status, &mut [])
                .unwrap();
        }
        q
    }
}
