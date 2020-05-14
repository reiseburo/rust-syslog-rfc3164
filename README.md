This module implements an [RFC 3164](https://tools.ietf.org/html/rfc3164) IETF Syslog Protocol parser in Rust. The code is a modified fork of the [Roguelazer's more complex 5424 parser](https://github.com/Roguelazer/rust-syslog-rfc5424).

[![Build Status](https://travis-ci.org/xrl/rust-syslog-rfc3164.svg?branch=master)](https://travis-ci.org/Roguelazer/rust-syslog-rfc5424)

[Documentation](https://docs.rs/syslog_rfc3164/)

This tool supports serializing the parsed messages using serde.

## Performance

On a recent system<sup>[1](#sysfootnote)</sup>, a release build takes approximately 8µs to parse an average message and approximately 300ns to parse the smallest legal message. Debug timings are a bit worse -- about 60µs for an average message and about 8µs for the minimal message. A single-threaded Syslog server should be able to parse at least 100,000 messages/s, as long as you run a separate thread for the parser.
