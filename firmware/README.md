# AirForceOne lite

## Features
* SCD4x sensor + SHT41 sensor (dead on both prototype boards, therefore unsupported)
* Data logging
* Bluetooth data transport (both GATT and ADV)
    * reading of historic data over GATT
* USB serial data tansport (immediate data only)

```
cargo run --release  | grep --line-buffered "data:" > output.txt
cat output.txt | cut -d ":" -f2 > output.csv
```
