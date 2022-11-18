use serde::ser::SerializeTuple;
use serde::{Deserializer, Serialize, Serializer};

use usbd_hid::descriptor::{AsInputReport, SerializedDescriptor};
use usbd_hid_macros::gen_hid_descriptor;

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = 0x77, usage = 0x01) = {
        (usage = 0x55,) = {
            #[item_settings data,variable,absolute] co2=input;
        };
    }
)]
pub struct AirQualityReport {
    pub co2: u16,
}

fn main() {
    let api = hidapi::HidApi::new().unwrap();
    // Print out information about all connected devices
    for device in api.device_list() {
        println!("{:#?}", device);
    }

    let (VID, PID) = (0xc0de, 0xcafe);
    let dev = api.open(VID, PID).unwrap();

    dev.send_feature_report(&[0; 20])
        .expect("Feature report failed");

    println!(
        "Manufacurer:\t{:?}",
        dev.get_manufacturer_string()
            .expect("Failed to read manufacurer string")
    );
    println!(
        "Product:\t{:?}",
        dev.get_product_string()
            .expect("Failed to read product string")
    );
    println!(
        "Serial number:\t{:?}",
        dev.get_serial_number_string()
            .expect("Failed to read serial number")
    );

    let mut buf = [0u8; 20];
    loop {
        let res = dev.read(&mut buf[..]).unwrap();
        println!("Read: {:?}", &buf[..res]);
    }
}
