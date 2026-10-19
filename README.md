# laden

PSI + rusage + EMA latency telemetry for load-aware schedulers.

`laden` gathers the signals a scheduler needs to place work on the least-loaded
host or agent: kernel pressure-stall information, run-queue load averages, this
process's peak memory footprint, and a smoothed view of upstream latency and
health. Parsing is split from I/O, so every `/proc` format has a pure function
you can drive from fixtures.

- Zero required dependencies beyond `thiserror`; `libc` only when you opt into
  `rusage`.
- Pure parsers (`parse_pressure`, `parse_loadavg`) that never touch `/proc`.
- Additive-only public API from 1.0 onward.
- MSRV: Rust 1.95.

## Modules

- `psi` — parse `/proc/pressure/{cpu,memory,io}` into `some`/`full` averages
  (`avg10`/`avg60`/`avg300` + `total`). `Psi::read()` samples the live files;
  `parse_pressure` is a pure function over the file contents. Missing files
  (no `CONFIG_PSI=y`, non-Linux hosts) surface as `PsiError::Unsupported`.
- `load` — parse `/proc/loadavg` into a `Load` (1/5/15-minute averages plus
  `running`/`total` task counts). `Load::read()` samples the live file,
  `parse_loadavg` is pure, and `cpu_count()` normalizes the averages per core.
- `rusage` *(feature `rusage`)* — `max_rss_kb()` reports peak RSS via
  `getrusage`, with the macOS bytes-to-kibibytes adjustment.
- `ema` — `Ema` exponential moving average, plus a `Health` tracker
  that smooths `record_latency` samples and derives a `Status`
  (`Active` / `Degraded` / `Down`) from consecutive failures.

## Features

| Feature  | Default | Enables                                                              |
| -------- | ------- | -------------------------------------------------------------------- |
| `rusage` | no      | The `rusage` module and its `libc` dependency; the crate's only `unsafe`. |

## Usage

Read the current pressure and load:

```rust,no_run
use laden::{Load, Psi};

let psi = Psi::read()?;
println!("cpu some avg10 = {}", psi.cpu.some.avg10);

let load = Load::read()?;
println!("1-min load per cpu = {:.2}", load.per_cpu_peak());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Track upstream health from latency samples and failures:

```rust
use laden::{Health, Status};

let mut health = Health::with_thresholds(2, 5);
health.record_latency(14.0);
assert_eq!(health.status(), Status::Active);

health.record_failure();
health.record_failure();
assert_eq!(health.status(), Status::Degraded);
```

Parse fixtures without touching `/proc`:

```rust
use laden::parse_loadavg;

let load = parse_loadavg("0.52 0.58 0.59 2/1234 56789").unwrap();
assert_eq!(load.running, 2);
```

## License

EUPL-1.2. See [LICENSE](./LICENSE).
