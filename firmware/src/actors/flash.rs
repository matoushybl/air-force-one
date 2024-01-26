use adafruit_feather_nrf52::EXTERNAL_FLASH_SIZE;
use ector::{actor, Actor, Address, Inbox};

use embassy_nrf::qspi::Qspi;
use embassy_time::{Duration, Timer};

use afo_shared::AirQuality;

// co2 - 2B, PM1.0 - 1B, PM2.5 1B, temp 1B, humi 1B, VOC 2 B = 8B
const ENTRY_LEN: usize = 8;
struct WritingIndex {
    offset: usize,
}

impl WritingIndex {
    fn new() -> Self {
        Self { offset: 0 }
    }

    fn next(&mut self) -> usize {
        let next = self.offset;

        self.offset += ENTRY_LEN;

        next
    }
}

pub struct Flash {
    qspi: Qspi<
        'static,
        embassy_nrf::peripherals::QSPI,
        { adafruit_feather_nrf52::EXTERNAL_FLASH_SIZE },
    >,
    index: Option<WritingIndex>,
}

impl Flash {
    pub fn new(
        qspi: Qspi<
            'static,
            embassy_nrf::peripherals::QSPI,
            { adafruit_feather_nrf52::EXTERNAL_FLASH_SIZE },
        >,
    ) -> Self {
        Self { qspi, index: None }
    }
}

pub enum LogCommand {
    EnableLogging(bool),
    LogValue(AirQuality),
}

#[actor]
impl Actor for Flash {
    type Message<'m> = LogCommand;

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        {
            let mut raw = AlignedRawEntry([0u8; ENTRY_LEN]);
            defmt::unwrap!(self.qspi.read(0, &mut raw.0).await);
            defmt::error!("data: {:x}", raw.0);

            let mut index = WritingIndex::new();
            let mut n = 1;
            loop {
                let addr = index.next();
                defmt::unwrap!(defmt::unwrap!(
                    embassy_time::with_timeout(
                        Duration::from_secs(1),
                        self.qspi.read(addr, &mut raw.0)
                    )
                    .await
                ));
                // defmt::error!("data: {:x}", raw.0);
                let _co2 = u16::from_le_bytes((raw.0[..2]).try_into().unwrap());
                let _voc = u16::from_le_bytes(raw.0[6..].try_into().unwrap());
                // defmt::error!(
                //     "data: {},{},{},{},{},{}",
                //     co2,
                //     raw.0[2],
                //     raw.0[3],
                //     raw.0[4],
                //     raw.0[5],
                //     voc
                // );

                Timer::after(Duration::from_millis(1)).await;
                if raw.0[0] == 0xff && raw.0[1] == 0xff && raw.0[2] == 0xff && raw.0[3] == 0xff {
                    defmt::error!("read end 0x{:x} - {}", addr, n);
                    break;
                }
                n += 1;
            }
        }
        loop {
            match inbox.next().await {
                LogCommand::EnableLogging(enable) => {
                    if enable {
                        defmt::info!("Erasing external flash");

                        for i in 0..(EXTERNAL_FLASH_SIZE / 4096) {
                            defmt::unwrap!(defmt::unwrap!(
                                embassy_time::with_timeout(
                                    Duration::from_secs(1),
                                    self.qspi.erase(i * 4096),
                                )
                                .await
                            ));
                            defmt::info!("Erasing external flash {}", i);
                            Timer::after(Duration::from_millis(1)).await
                        }

                        defmt::info!("Erasing done.");
                        self.index = Some(WritingIndex::new());
                    } else {
                        self.index = None;
                    }
                }
                LogCommand::LogValue(_air_quality) => {
                    if let Some(_index) = self.index.as_mut() {
                        // let flash_offset = index.next();
                        // let mut raw = AlignedRawEntry([0u8; ENTRY_LEN]);
                        // raw.0[..2].copy_from_slice(&(air_quality.co2.0 as u16).to_le_bytes());
                        // raw.0[2] = air_quality.pm.mass_10 as u8;
                        // raw.0[3] = air_quality.pm.mass_25 as u8;
                        // raw.0[4] = (air_quality.temperature.0 * 10.0) as u8;
                        // raw.0[5] = (air_quality.humidity.0) as u8;
                        // raw.0[6..].copy_from_slice(&air_quality.voc.index.to_le_bytes());
                        //
                        // defmt::unwrap!(defmt::unwrap!(
                        //     embassy_time::with_timeout(
                        //         Duration::from_millis(100),
                        //         self.qspi.write(flash_offset, &raw.0)
                        //     )
                        //     .await
                        // ));
                        defmt::error!("Flash logging unavailable.");
                    }
                }
            }
        }
    }
}

#[repr(C, align(4))]
pub struct AlignedRawEntry(pub [u8; ENTRY_LEN]);
