//! Names where a fatal signal hit, since the core dump the system keeps leaves out the registers.
//!
//! Installed before the core loads, so it is the handler a core's own fault filter chains to when
//! it doesn't claim a fault, and the one that sees every fault outside the core's threads.

use libc::{c_int, c_void, siginfo_t};
use std::fmt::Write;
use std::sync::OnceLock;

const SIGNALS: [c_int; 3] = [libc::SIGSEGV, libc::SIGBUS, libc::SIGILL];
static PREVIOUS: [OnceLock<libc::sigaction>; 3] =
    [OnceLock::new(), OnceLock::new(), OnceLock::new()];

/// A fixed buffer to format into, as nothing in a signal handler may allocate.
struct Line {
    bytes: [u8; 512],
    len: usize,
}

impl Write for Line {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let room = self.bytes.len() - self.len;
        let take = s.len().min(room);
        self.bytes[self.len..self.len + take].copy_from_slice(&s.as_bytes()[..take]);
        self.len += take;
        Ok(())
    }
}

pub fn install() {
    for (&signal, previous) in SIGNALS.iter().zip(&PREVIOUS) {
        // SAFETY: a zeroed sigaction is a valid empty one, and the handler only calls
        // async-signal-safe functions.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_fault as *const () as usize;
            // The previous handler may need to raise the signal again.
            action.sa_flags = libc::SA_SIGINFO | libc::SA_NODEFER;
            libc::sigemptyset(&mut action.sa_mask);
            let mut old: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(signal, &action, &mut old) == 0 {
                let _ = previous.set(old);
            }
        }
    }
}

unsafe extern "C" fn on_fault(signal: c_int, info: *mut siginfo_t, context: *mut c_void) {
    let mut line = Line {
        bytes: [0; 512],
        len: 0,
    };
    // SAFETY: the kernel passes a valid siginfo_t and ucontext_t to a SA_SIGINFO handler.
    unsafe { describe(&mut line, signal, info, context) };
    let _ = line.write_str("\n");
    // SAFETY: the buffer holds `len` initialized bytes.
    unsafe { libc::write(libc::STDERR_FILENO, line.bytes.as_ptr().cast(), line.len) };

    let Some(index) = SIGNALS.iter().position(|&s| s == signal) else {
        return;
    };
    let Some(previous) = PREVIOUS[index].get() else {
        return;
    };
    // SAFETY: the stored action is what sigaction reported for this signal.
    unsafe {
        if previous.sa_flags & libc::SA_SIGINFO != 0 && previous.sa_sigaction != 0 {
            let chained: unsafe extern "C" fn(c_int, *mut siginfo_t, *mut c_void) =
                std::mem::transmute(previous.sa_sigaction);
            chained(signal, info, context);
        } else if previous.sa_sigaction == libc::SIG_DFL || previous.sa_sigaction == libc::SIG_IGN {
            // The instruction runs again and faults, now with nothing in the way of a core dump.
            libc::signal(signal, libc::SIG_DFL);
        } else {
            let chained: unsafe extern "C" fn(c_int) = std::mem::transmute(previous.sa_sigaction);
            chained(signal);
        }
    }
}

unsafe fn describe(line: &mut Line, signal: c_int, info: *mut siginfo_t, context: *mut c_void) {
    // SAFETY: see on_fault.
    let (code, address) = unsafe { ((*info).si_code, (*info).si_addr() as usize) };
    let _ = write!(
        line,
        "fatal signal {signal} code={code} address={address:#x}"
    );
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        // SAFETY: see on_fault.
        let registers = unsafe { &(*context.cast::<libc::ucontext_t>()).uc_mcontext.gregs };
        let rip = registers[libc::REG_RIP as usize] as usize;
        let _ = write!(line, " rip={rip:#x}");
        // SAFETY: dladdr only reads the loader's tables.
        unsafe {
            let mut module: libc::Dl_info = std::mem::zeroed();
            if libc::dladdr(rip as *const c_void, &mut module) != 0 && !module.dli_fname.is_null() {
                let name = std::ffi::CStr::from_ptr(module.dli_fname).to_bytes();
                let name = name.rsplit(|&b| b == b'/').next().unwrap_or(name);
                let _ = write!(
                    line,
                    " ({}+{:#x})",
                    String::from_utf8_lossy(name),
                    rip - module.dli_fbase as usize
                );
            }
        }
        for (name, register) in [
            ("rax", libc::REG_RAX),
            ("rbx", libc::REG_RBX),
            ("rcx", libc::REG_RCX),
            ("rdx", libc::REG_RDX),
            ("rsp", libc::REG_RSP),
        ] {
            let _ = write!(line, " {name}={:#x}", registers[register as usize]);
        }
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    let _ = context;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::fd::FromRawFd;

    #[test]
    fn a_fault_is_described_then_still_kills_the_process() {
        let mut pipe = [0; 2];
        // SAFETY: the child only calls async-signal-safe functions before it faults.
        unsafe {
            assert_eq!(libc::pipe(pipe.as_mut_ptr()), 0);
            let child = libc::fork();
            if child == 0 {
                libc::dup2(pipe[1], libc::STDERR_FILENO);
                install();
                std::ptr::null_mut::<u8>().write_volatile(1);
                libc::_exit(0);
            }
            libc::close(pipe[1]);
            let mut status = 0;
            libc::waitpid(child, &mut status, 0);
            let mut output = String::new();
            std::fs::File::from_raw_fd(pipe[0])
                .read_to_string(&mut output)
                .unwrap();
            assert!(libc::WIFSIGNALED(status));
            assert_eq!(libc::WTERMSIG(status), libc::SIGSEGV);
            assert!(output.starts_with("fatal signal 11 code="), "{output}");
            assert!(output.contains(" address=0x0 "), "{output}");
            #[cfg(target_arch = "x86_64")]
            assert!(output.contains(" rbx=0x"), "{output}");
        }
    }
}
