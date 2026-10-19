# laden

PSI + RSS + EMA latency telemetry for load-aware schedulers.

`laden` gathers the signals a scheduler needs to place work on the least-loaded
host or agent. That covers kernel pressure-stall information, run-queue load
averages, this process's peak memory footprint, and a smoothed view of upstream
latency and health. Parsing is split from I/O, so every `/proc` format has a pure
function you can drive from fixtures.

The only dependency is `flub`, making it a very lightweight dependency.

- `psi` parses `/proc/pressure/{cpu,memory,io}`. Missing files (possibly due to
  no `CONFIG_PSI=y`, or on a non-Linux hosts) surface as
  `psi::Error::Unsupported`.
- `load` parses `/proc/loadavg`, and `cpu_count()` normalizes the averages per
  core.
- `rss` reports peak RSS via `getrusage`, or `K32GetProcessMemoryInfo` on
  Windows.
- `ema` provides an exponential moving average and a `Health` tracker that
  derives `Active`, `Degraded`, or `Down` from consecutive failures.

## Usage

Read the current pressure and load.

```rust,no_run
use laden::{load::Load, psi::Psi};

let psi = Psi::read()?;
println!("cpu some avg10 = {}", psi.cpu.some.avg10);

let load = Load::read()?;
println!("1-min load per cpu = {:.2}", load.per_cpu_peak());
```

Track upstream health from latency samples and failures.

```rust
use laden::ema::{Alpha, Health, Status, Thresholds};

let mut health = Health::new(Alpha::DEFAULT, Thresholds { degraded: 2, down: 5 });
health.record_latency(14.0);
assert_eq!(health.status(), Status::Active);

health.record_failure();
health.record_failure();
assert_eq!(health.status(), Status::Degraded);
```

Parse a fixture.

```rust
use laden::load::parse_loadavg;

let load = parse_loadavg("0.52 0.58 0.59 2/1234 56789").unwrap();
assert_eq!(load.running, 2);
```

## License

EUPL-1.2. See [LICENSE](./LICENSE).
