//! Peak resident set size of this process, via `getrusage`.

use std::{io, mem::MaybeUninit};

/// Peak resident set size of this process in kibibytes.
///
/// # Errors
///
/// Returns the OS error when `getrusage` fails, or an other-kind error when
/// the kernel reports a negative `ru_maxrss`.
pub fn max_rss_kb() -> io::Result<u64> {
  let mut usage = MaybeUninit::<libc::rusage>::uninit();
  // SAFETY: `usage.as_mut_ptr()` is a valid, non-null pointer to writable
  // storage for `libc::rusage`. `getrusage` fully initializes it on success,
  // which is checked before `assume_init`.
  let result =
    unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
  if result == -1 {
    return Err(io::Error::last_os_error());
  }
  // SAFETY: a successful `getrusage` call initializes the entire `rusage`.
  let usage = unsafe { usage.assume_init() };
  let rss = u64::try_from(usage.ru_maxrss)
    .map_err(|_| io::Error::other("getrusage returned a negative ru_maxrss"))?;
  // Linux reports ru_maxrss in kibibytes; macOS reports bytes.
  if cfg!(target_os = "macos") {
    Ok(rss / 1024)
  } else {
    Ok(rss)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reports_nonzero_rss() {
    let kb = max_rss_kb().expect("getrusage succeeds");
    assert!(kb > 0);
  }
}
