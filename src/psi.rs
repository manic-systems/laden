//! Pressure Stall Information from `/proc/pressure/{cpu,memory,io}`.

use std::{
   fs,
   io,
};

/// An error reading or parsing pressure-stall information.
#[derive(Debug, flub::Error)]
pub enum Error {
   /// The pressure files are absent, as on a kernel built without
   /// `CONFIG_PSI=y` or a non-Linux host.
   #[error("pressure information is unavailable on this system")]
   Unsupported,
   /// Reading the file failed for a reason other than its absence.
   #[error("reading {path}")]
   Io {
      /// The `/proc/pressure/*` path that failed to read.
      path:   &'static str,
      /// The underlying I/O error.
      source: io::Error,
   },
   /// The file lacked a `some` line, or a line was missing a field.
   #[error("malformed pressure data")]
   Malformed,
}

/// One pressure line: the share of time tasks stalled on a resource, averaged
/// over the trailing 10-, 60-, and 300-second windows, plus the lifetime total
/// stall time in microseconds.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Line {
   /// Percentage of wall-clock time stalled over the last 10 seconds.
   pub avg10:  f64,
   /// Percentage of wall-clock time stalled over the last 60 seconds.
   pub avg60:  f64,
   /// Percentage of wall-clock time stalled over the last 300 seconds.
   pub avg300: f64,
   /// Cumulative stall time in microseconds since boot.
   pub total:  u64,
}

/// The `some` and (where the kernel reports it) `full` lines for one resource.
///
/// `some` tracks time at least one task was stalled; `full` tracks time *every*
/// runnable task was stalled. The CPU file has no `full` line, so [`full`] is
/// `None` there.
///
/// [`full`]: Stat::full
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stat {
   /// Pressure while at least one task was stalled on the resource.
   pub some: Line,
   /// Pressure while every runnable task was stalled; absent for CPU.
   pub full: Option<Line>,
}

/// Pressure across all three tracked resources.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Psi {
   /// CPU pressure (`some` only).
   pub cpu:    Stat,
   /// Memory pressure.
   pub memory: Stat,
   /// I/O pressure.
   pub io:     Stat,
}

impl Psi {
   /// Read all three pressure files.
   ///
   /// # Errors
   ///
   /// Returns [`Error::Unsupported`] when the files are absent (a kernel
   /// without `CONFIG_PSI=y`, or a non-Linux host), and [`Error::Io`] or
   /// [`Error::Malformed`] on other read or parse failures.
   #[inline]
   pub fn read() -> Result<Self, Error> {
      Ok(Self {
         cpu:    read_stat("/proc/pressure/cpu")?,
         memory: read_stat("/proc/pressure/memory")?,
         io:     read_stat("/proc/pressure/io")?,
      })
   }
}

/// Read and parse one `/proc/pressure/*` file, mapping its absence to
/// [`Error::Unsupported`].
fn read_stat(path: &'static str) -> Result<Stat, Error> {
   let text = match fs::read_to_string(path) {
      Ok(text) => text,
      Err(err) if err.kind() == io::ErrorKind::NotFound => {
         return Err(Error::Unsupported);
      },
      Err(source) => return Err(Error::Io { path, source }),
   };
   parse_pressure(&text)
}

/// Parse the contents of one `/proc/pressure/*` file.
///
/// # Errors
///
/// Returns [`Error::Malformed`] when the `some` line is absent or any line
/// lacks one of `avg10`, `avg60`, `avg300`, or `total`.
#[inline]
pub fn parse_pressure(text: &str) -> Result<Stat, Error> {
   let mut some = None;
   let mut full = None;
   for line in text.lines() {
      if let Some(rest) = line.strip_prefix("some ") {
         some = Some(parse_line(rest).ok_or(Error::Malformed)?);
      } else if let Some(rest) = line.strip_prefix("full ") {
         full = Some(parse_line(rest).ok_or(Error::Malformed)?);
      }
   }
   Ok(Stat {
      some: some.ok_or(Error::Malformed)?,
      full,
   })
}

/// Parse the `key=value` fields after a `some` or `full` prefix, or `None` if
/// any of the four is missing or unparseable.
fn parse_line(fields: &str) -> Option<Line> {
   let (mut avg10, mut avg60, mut avg300, mut total) = (None, None, None, None);
   for kv in fields.split_whitespace() {
      let Some((key, value)) = kv.split_once('=') else {
         continue;
      };
      match key {
         "avg10" => avg10 = Some(value.parse().ok()?),
         "avg60" => avg60 = Some(value.parse().ok()?),
         "avg300" => avg300 = Some(value.parse().ok()?),
         "total" => total = Some(value.parse().ok()?),
         _ => {},
      }
   }
   Some(Line {
      avg10:  avg10?,
      avg60:  avg60?,
      avg300: avg300?,
      total:  total?,
   })
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn parses_cpu_some_line() {
      let stat = parse_pressure("some avg10=0.00 avg60=0.00 avg300=0.00 total=12345")
         .expect("some line parses");
      assert_eq!(stat.some, Line {
         avg10:  0.0,
         avg60:  0.0,
         avg300: 0.0,
         total:  12345,
      });
      assert!(stat.full.is_none());
   }

   #[test]
   fn parses_some_and_full() {
      let stat = parse_pressure(
         "some avg10=5.00 avg60=3.00 avg300=2.00 total=100\nfull avg10=1.00 avg60=1.00 \
          avg300=1.00 total=42",
      )
      .expect("both lines parse");
      assert_eq!(stat.some, Line {
         avg10:  5.0,
         avg60:  3.0,
         avg300: 2.0,
         total:  100,
      });
      assert_eq!(stat.full.expect("full present").total, 42);
   }

   #[test]
   fn rejects_missing_some() {
      assert!(matches!(
         parse_pressure("full avg10=1.00 avg60=1.00 avg300=1.00 total=1"),
         Err(Error::Malformed)
      ));
   }

   #[test]
   fn parses_memory_some_and_full() {
      let stat = parse_pressure(
         "some avg10=2.50 avg60=1.25 avg300=0.75 total=900\nfull avg10=0.50 avg60=0.25 \
          avg300=0.10 total=120",
      )
      .expect("memory lines parse");
      assert_eq!(stat.some, Line {
         avg10:  2.5,
         avg60:  1.25,
         avg300: 0.75,
         total:  900,
      });
      assert_eq!(
         stat.full,
         Some(Line {
            avg10:  0.5,
            avg60:  0.25,
            avg300: 0.1,
            total:  120,
         })
      );
   }

   #[test]
   fn tolerates_trailing_whitespace() {
      let stat = parse_pressure("some avg10=1.00 avg60=1.00 avg300=1.00 total=7   \n")
         .expect("trailing whitespace tolerated");
      assert_eq!(stat.some.total, 7);
   }

   #[test]
   fn rejects_some_line_without_fields() {
      assert!(matches!(
         parse_pressure("some garbage-with-no-equals"),
         Err(Error::Malformed)
      ));
   }

   #[test]
   fn ignores_unknown_keys() {
      let stat = parse_pressure("some avg10=1.00 avg60=1.00 avg300=1.00 extra=9 total=3")
         .expect("unknown key ignored");
      assert_eq!(stat.some.total, 3);
   }
}
