//! Peak resident set size of this process.

use std::io;

/// Peak resident set size of this process in kibibytes.
///
/// # Errors
///
/// Returns the OS error when the platform query fails, an
/// [`io::ErrorKind::Unsupported`] error on platforms without one, or an
/// other-kind error when the kernel reports a negative `ru_maxrss`.
pub fn max_rss_kb() -> io::Result<u64> {
  sys::max_rss_kb()
}

#[cfg(any(
  target_os = "linux",
  target_os = "android",
  target_vendor = "apple",
  target_os = "freebsd",
  target_os = "dragonfly",
  target_os = "openbsd",
  target_os = "netbsd",
  target_os = "illumos",
  target_os = "solaris",
))]
mod sys {
  use std::{
    ffi::{c_int, c_long},
    io,
  };

  #[cfg(any(
    target_os = "openbsd",
    target_os = "netbsd",
    all(target_os = "freebsd", not(target_arch = "x86")),
    all(target_os = "linux", target_arch = "riscv32"),
  ))]
  type TimeT = i64;
  #[cfg(not(any(
    target_os = "openbsd",
    target_os = "netbsd",
    all(target_os = "freebsd", not(target_arch = "x86")),
    all(target_os = "linux", target_arch = "riscv32"),
  )))]
  type TimeT = c_long;

  #[cfg(any(target_vendor = "apple", target_os = "netbsd"))]
  type SusecondsT = i32;
  #[cfg(all(target_os = "linux", target_arch = "riscv32"))]
  type SusecondsT = i64;
  #[cfg(not(any(
    target_vendor = "apple",
    target_os = "netbsd",
    all(target_os = "linux", target_arch = "riscv32"),
  )))]
  type SusecondsT = c_long;

  #[repr(C)]
  struct Timeval {
    _sec:  TimeT,
    _usec: SusecondsT,
  }

  #[repr(C)]
  struct Rusage {
    _utime:    Timeval,
    _stime:    Timeval,
    ru_maxrss: c_long,
    _rest:     [c_long; 32],
  }

  const RUSAGE_SELF: c_int = 0;

  unsafe extern "C" {
    #[cfg_attr(target_os = "netbsd", link_name = "__getrusage50")]
    fn getrusage(who: c_int, usage: *mut Rusage) -> c_int;
  }

  pub fn max_rss_kb() -> io::Result<u64> {
    let mut usage = Rusage {
      _utime:    Timeval { _sec: 0, _usec: 0 },
      _stime:    Timeval { _sec: 0, _usec: 0 },
      ru_maxrss: 0,
      _rest:     [0; 32],
    };
    // SAFETY: `Rusage` matches this target's `struct rusage` through
    // `ru_maxrss`, and `_rest` pads it past the largest layout any supported
    // libc defines, so `getrusage` writes only inside `usage`.
    if unsafe { getrusage(RUSAGE_SELF, &raw mut usage) } == -1 {
      return Err(io::Error::last_os_error());
    }
    let rss = u64::try_from(usage.ru_maxrss).map_err(|_| {
      io::Error::other("getrusage returned a negative ru_maxrss")
    })?;
    // Linux reports ru_maxrss in kibibytes; macOS reports bytes.
    if cfg!(target_vendor = "apple") {
      Ok(rss / 1024)
    } else {
      Ok(rss)
    }
  }
}

#[cfg(windows)]
mod sys {
  use std::{ffi::c_void, io};

  #[repr(C)]
  struct ProcessMemoryCounters {
    cb:                    u32,
    _page_fault_count:     u32,
    peak_working_set_size: usize,
    _rest:                 [usize; 7],
  }

  #[link(name = "kernel32")]
  unsafe extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn K32GetProcessMemoryInfo(
      process: *mut c_void,
      counters: *mut ProcessMemoryCounters,
      cb: u32,
    ) -> i32;
  }

  #[allow(
    clippy::cast_possible_truncation,
    reason = "PROCESS_MEMORY_COUNTERS is 80 bytes at most"
  )]
  const CB: u32 = size_of::<ProcessMemoryCounters>() as u32;

  pub fn max_rss_kb() -> io::Result<u64> {
    let mut counters = ProcessMemoryCounters {
      cb:                    CB,
      _page_fault_count:     0,
      peak_working_set_size: 0,
      _rest:                 [0; 7],
    };
    // SAFETY: `ProcessMemoryCounters` matches `PROCESS_MEMORY_COUNTERS`, `cb`
    // is its size, and `GetCurrentProcess` returns a pseudo-handle that needs
    // no closing.
    let ok = unsafe {
      K32GetProcessMemoryInfo(GetCurrentProcess(), &raw mut counters, CB)
    };
    if ok == 0 {
      return Err(io::Error::last_os_error());
    }
    Ok(counters.peak_working_set_size as u64 / 1024)
  }
}

#[cfg(not(any(
  windows,
  target_os = "linux",
  target_os = "android",
  target_vendor = "apple",
  target_os = "freebsd",
  target_os = "dragonfly",
  target_os = "openbsd",
  target_os = "netbsd",
  target_os = "illumos",
  target_os = "solaris",
)))]
mod sys {
  use std::io;

  pub fn max_rss_kb() -> io::Result<u64> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      "peak RSS is unavailable on this platform",
    ))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reports_nonzero_rss() {
    let kb = max_rss_kb().expect("peak RSS query succeeds");
    assert!(kb > 0);
  }
}
