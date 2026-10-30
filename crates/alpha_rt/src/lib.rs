// Alpha Precompiled C-ABI Static Runtime Library (`alpha_rt`).
// Provides core runtime capabilities linked directly by LLVM / Clang without rustc.

use std::ffi::{CStr, CString};
use std::fs;
use std::io::Write;
use std::os::raw::{c_char, c_void};
use std::path::Path;
use std::time::Instant;

// =========================================================================
// Dynamic Vector (vect<T>)
// =========================================================================

pub struct AlphaVect {
    pub items: Vec<*mut c_void>,
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_vect_new() -> *mut AlphaVect {
    let v = Box::new(AlphaVect { items: Vec::new() });
    Box::into_raw(v)
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_vect_push(v: *mut AlphaVect, item: *mut c_void) {
    if !v.is_null() {
        unsafe {
            (*v).items.push(item);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_vect_len(v: *const AlphaVect) -> i64 {
    if v.is_null() {
        0
    } else {
        unsafe { (*v).items.len() as i64 }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_vect_get(v: *const AlphaVect, index: i64) -> *mut c_void {
    if v.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        // Alpha uses 1-based indexing
        let idx = (index - 1) as usize;
        if idx < (*v).items.len() {
            (&(*v).items)[idx]
        } else {
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_vect_free(v: *mut AlphaVect) {
    if !v.is_null() {
        unsafe {
            drop(Box::from_raw(v));
        }
    }
}

// =========================================================================
// Standard I/O & Terminal Painting & Formatting
// =========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_format_str(fmt: *const c_char, arg: *const c_char) -> *mut c_char {
    if fmt.is_null() {
        return std::ptr::null_mut();
    }
    let fmt_str = unsafe { CStr::from_ptr(fmt).to_str().unwrap_or("") };
    let arg_str = if !arg.is_null() {
        unsafe { CStr::from_ptr(arg).to_str().unwrap_or("") }
    } else {
        ""
    };
    let formatted = fmt_str.replace("{}", arg_str);
    CString::new(formatted).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_print(s: *const c_char) {
    if !s.is_null() {
        unsafe {
            if let Ok(str_slice) = CStr::from_ptr(s).to_str() {
                print!("{}", str_slice);
                let _ = std::io::stdout().flush();
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_println(s: *const c_char) {
    if !s.is_null() {
        unsafe {
            if let Ok(str_slice) = CStr::from_ptr(s).to_str() {
                println!("{}", str_slice);
            }
        }
    } else {
        println!();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_cli_paint(text: *const c_char, hex_color: *const c_char) -> *mut c_char {
    if text.is_null() {
        return std::ptr::null_mut();
    }

    let text_str = unsafe {
        CStr::from_ptr(text).to_str().unwrap_or("")
    };

    let hex_str = if !hex_color.is_null() {
        unsafe { CStr::from_ptr(hex_color).to_str().unwrap_or("").trim_start_matches('#') }
    } else {
        ""
    };

    let result = if hex_str.len() == 6 {
        let r = u8::from_str_radix(&hex_str[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex_str[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&hex_str[4..6], 16).unwrap_or(255);
        format!("\x1b[38;2;{};{};{}m{}\x1b[0m", r, g, b, text_str)
    } else {
        text_str.to_string()
    };

    CString::new(result).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_cli_arguments() -> *mut AlphaVect {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let v = alpha_rt_vect_new();
    for a in args {
        if let Ok(c_str) = CString::new(a) {
            alpha_rt_vect_push(v, c_str.into_raw() as *mut c_void);
        }
    }
    v
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_str_chars(s: *const c_char) -> *mut AlphaVect {
    let v = alpha_rt_vect_new();
    if !s.is_null() {
        unsafe {
            if let Ok(str_slice) = CStr::from_ptr(s).to_str() {
                for c in str_slice.chars() {
                    let mut buf = [0u8; 4];
                    let c_str = c.encode_utf8(&mut buf);
                    if let Ok(c_string) = CString::new(&*c_str) {
                        alpha_rt_vect_push(v, c_string.into_raw() as *mut c_void);
                    }
                }
            }
        }
    }
    v
}

// =========================================================================
// Core / Io (Filesystem)
// =========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_io_exists(path: *const c_char) -> bool {
    if path.is_null() {
        return false;
    }
    unsafe {
        if let Ok(p) = CStr::from_ptr(path).to_str() {
            Path::new(p).exists()
        } else {
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_io_read(path: *const c_char) -> *mut c_char {
    if path.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        if let Ok(p) = CStr::from_ptr(path).to_str() {
            match fs::read_to_string(p) {
                Ok(content) => CString::new(content).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut()),
                Err(_) => std::ptr::null_mut(),
            }
        } else {
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_io_write(path: *const c_char, content: *const c_char) -> bool {
    if path.is_null() || content.is_null() {
        return false;
    }
    unsafe {
        if let (Ok(p), Ok(c)) = (CStr::from_ptr(path).to_str(), CStr::from_ptr(content).to_str()) {
            fs::write(p, c).is_ok()
        } else {
            false
        }
    }
}

// =========================================================================
// Core / Time
// =========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_time_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn alpha_rt_time_benchmark(f: Option<extern "C" fn()>) -> i64 {
    if let Some(func) = f {
        let t0 = Instant::now();
        func();
        t0.elapsed().as_millis() as i64
    } else {
        0
    }
}
