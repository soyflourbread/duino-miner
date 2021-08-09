# duino-miner

A multiplexed CPU miner for Duinocoin (duino-coin),
*if you know what I am talking about*.

This high-performance miner is implemented in asynchronous Rust, from scratch.
This makes it several times faster than the official miner,
and highly efficient even on non-conventional mining hardwares
such as a Raspberry Pi. (Again, if you know what I am talking about.)

To build this project, execute

```sh
cargo build --release
```

To generate a config file for the miner, execute

```sh
duino-miner generate -u my_username --device-type=AVR --firmware="Official AVR Miner v2.6" --device-name-prefix "avr-" --target-rate 182
```

To run the miner from your config file, execute

```sh
duino-miner run
```
