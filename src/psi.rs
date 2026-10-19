//! Pressure Stall Information from `/proc/pressure/{cpu,memory,io}`.

use std::{
   fs,
   io,
};

/// An error reading or parsing pressure-stall information.
#[derive(Debug, flub::Error)]
pub enum PsiError {
   /// The pressure files are absent, as on a kernel built without
   /// `CONFIG_PSI=y` or a non-Linux host.
   #[error("pressure information is unavailable on this system")]
   Unsupported,
   /// Reading the file failed for a reason other than its absence.
   #[error("reading {path}: {source}")]
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
pub struct PsiLine {
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
/// [`full`]: PsiStat::full
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PsiStat {
   /// Pressure while at least one task was stalled on the resource.
   pub some: PsiLine,
   /// Pressure while every runnable task was stalled; absent for CPU.
   pub full: Option<PsiLine>,
}

/// Pressure across all three tracked resources.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Psi {
   /// CPU pressure (`some` only).
   pub cpu:    PsiStat,
   /// Memory pressure.
   pub memory: PsiStat,
   /// I/O pressure.
   pub io:     PsiStat,
}

impl Psi {
   /// Read all three pressure files.
   ///
   /// # Errors
   ///
   /// Returns [`PsiError::Unsupported`] when the files are absent (a kernel
   /// without `CONFIG_PSI=y`, or a non-Linux host), and [`PsiError::Io`] or
   /// [`PsiError::Malformed`] on other read or parse failures.
   pub fn read() -> Result<Self, PsiError> {
      Ok(Self {
         cpu:    read_stat("/proc/pressure/cpu")?,
         memory: read_stat("/proc/pressure/memory")?,
         io:     read_stat("/proc/pressure/io")?,
      })
   }
}

fn read_stat(path: &'static str) -> Result<PsiStat, PsiError> {
   let text = match fs::read_to_string(path) {
      Ok(text) => text,
      Err(e) if e.kind() == io::ErrorKind::NotFound => {
         return Err(PsiError::Unsupported);
      },
      Err(source) => return Err(PsiError::Io { path, source }),
   };
   parse_pressure(&text)
}

/// Parse the contents of one `/proc/pressure/*` file.
///
/// # Errors
///
/// Returns [`PsiError::Malformed`] when the `some` line is absent or any line
/// lacks one of `avg10`, `avg60`, `avg300`, or `total`.
pub fn parse_pressure(text: &str) -> Result<PsiStat, PsiError> {
   let mut some = None;
   let mut full = None;
   for line in text.lines() {
      if let Some(rest) = line.strip_prefix("some ") {
         some = Some(parse_line(rest).ok_or(PsiError::Malformed)?);
      } else if let Some(rest) = line.strip_prefix("full ") {
         full = Some(parse_line(rest).ok_or(PsiError::Malformed)?);
      }
   }
   Ok(PsiStat {
      some: some.ok_or(PsiError::Malformed)?,
      full,
   })
}

fn parse_line(fields: &str) -> Option<PsiLine> {
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
   Some(PsiLine {
      avg10:  avg10?,
      avg60:  avg60?,
      avg300: avg300?,
      total:  total?,
   })
}

#[cfg(test)]
mod tests {
   // Fixtures use decimal literals that round-trip exactly to `f64`, so exact
   // equality is the correct assertion here.
   #![allow(clippy::float_cmp)]

   use super::*;

   #[test]
   fn parses_cpu_some_line() {
      let stat = parse_pressure("some avg10=0.00 avg60=0.00 avg300=0.00 total=12345")
         .expect("some line parses");
      assert_eq!(stat.some, PsiLine {
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
      assert_eq!(stat.some.avg10, 5.0);
      assert_eq!(stat.full.expect("full present").total, 42);
   }

   #[test]
   fn rejects_missing_some() {
      assert!(matches!(
         parse_pressure("full avg10=1.00 avg60=1.00 avg300=1.00 total=1"),
         Err(PsiError::Malformed)
      ));
   }

   #[test]
   fn parses_memory_some_and_full() {
      let stat = parse_pressure(
         "some avg10=2.50 avg60=1.25 avg300=0.75 total=900\nfull avg10=0.50 avg60=0.25 \
          avg300=0.10 total=120",
      )
      .expect("memory lines parse");
      assert_eq!(stat.some.avg10, 2.5);
      let full = stat.full.expect("full present");
      assert_eq!(full.avg300, 0.10);
      assert_eq!(full.total, 120);
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
         Err(PsiError::Malformed)
      ));
   }

   #[test]
   fn ignores_unknown_keys() {
      let stat = parse_pressure("some avg10=1.00 avg60=1.00 avg300=1.00 extra=9 total=3")
         .expect("unknown key ignored");
      assert_eq!(stat.some.total, 3);
   }
}
