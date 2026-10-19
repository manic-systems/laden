# laden

Pressure, load and memory signals for smart delegation.

## About

`laden` parses the kernel's pressure-stall information and run-queue load
averages, reports this process's peak RSS, and tracks the health of upstream
peers with an exponential moving average. The only dependency for `laden` is
`flub`, used for the error derives.

## Usage

You'll need Rust 1.97.0 (or above) and a Linux system. The latter is becasue
`psi` and `load` read `/proc`, so laden Linux-only for now. At least in
practice. When the file is missing, on a kernel without `CONFIG_PSI=y` or on
another OS, both return `Error::Unsupported`.

### API

Read the current pressure and load:

```rust,no_run
use laden::{load::Load, psi::Psi};

let psi = Psi::read()?;
println!("cpu some avg10 = {}", psi.cpu.some.avg10);

let load = Load::read()?;
println!("1-min load per cpu = {:.2}", load.per_cpu_peak());
```

Track an upstream peer from latency samples and consecutive failures. With the
default thresholds, status is `Degraded` after 3 consecutive failures and `Down`
after 10:

```rust
use laden::ema::{Alpha, Health, Status, Thresholds};

let mut health = Health::new(Alpha::DEFAULT, Thresholds { degraded: 2, down: 5 });
health.record_latency(14.0);
assert_eq!(health.status(), Status::Active);

health.record_failure();
health.record_failure();
assert_eq!(health.status(), Status::Degraded);
```

`rss` reports peak RSS through `getrusage` on Linux, Android, Apple and the
BSDs, and `K32GetProcessMemoryInfo` on Windows. On illumos and Solaris
`getrusage` always reports a zero `ru_maxrss`, so `rss` returns
`io::ErrorKind::Unsupported` there. `ema` is pure and platform-independent.

Parse a fixture directly:

```rust
use laden::load::parse_loadavg;

let load = parse_loadavg("0.52 0.58 0.59 2/1234 56789").unwrap();
assert_eq!(load.running, 2);
```

Parsing is split from I/O. `parse_pressure` and `parse_loadavg` are pure
functions over file contents, so tests can run them against fixtures instead of
`/proc`. The `Psi::read` and `Load::read` wrappers perform the file I/O.

## License

<!--markdownlint-disable MD033 MD059-->

[provided here]: https://interoperable-europe.ec.europa.eu/sites/default/files/custom-page/attachment/eupl_v1.2_en.pdf

This project is made available under European Union Public Licence (EUPL)
version 1.2. See [LICENSE](LICENSE) for more details on the exact conditions. An
online copy is [provided here].

<!--markdownlint-enable MD033 MD059-->
