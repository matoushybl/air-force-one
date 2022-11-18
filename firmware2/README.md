```
cargo run --release  | grep --line-buffered "data:" > output.txt
cat output.txt | cut -d ":" -f2 > output.csv
```
