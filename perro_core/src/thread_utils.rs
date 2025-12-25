/// Utility functions for thread management

use std::cell::RefCell;

thread_local! {
    static THREAD_NAME: RefCell<Option<String>> = RefCell::new(None);
}

/// Set the name of the current thread
/// This uses platform-specific APIs to name the thread, which will show up
/// in debuggers, profilers, and panic messages.
pub fn set_current_thread_name(name: &str) {
    // Store in thread-local for panic hook access
    THREAD_NAME.with(|tn| {
        *tn.borrow_mut() = Some(name.to_string());
    });
    
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        
        // Windows 10 version 1607+ supports SetThreadDescription
        // We'll use a dynamic call to avoid requiring a specific Windows SDK version
        let kernel32 = unsafe { 
            winapi::um::libloaderapi::GetModuleHandleA(
                b"kernel32.dll\0".as_ptr() as *const i8
            )
        };
        
        if !kernel32.is_null() {
            let set_thread_description = unsafe {
                winapi::um::libloaderapi::GetProcAddress(
                    kernel32,
                    b"SetThreadDescription\0".as_ptr() as *const i8
                )
            };
            
            if !set_thread_description.is_null() {
                // Convert Rust string to Windows wide string
                let wide: Vec<u16> = OsStr::new(name)
                    .encode_wide()
                    .chain(Some(0).into_iter())
                    .collect();
                
                // Call SetThreadDescription via function pointer
                type SetThreadDescriptionFn = unsafe extern "system" fn(
                    winapi::um::winnt::HANDLE,
                    *const u16,
                ) -> winapi::shared::winerror::HRESULT;
                
                let func: SetThreadDescriptionFn = unsafe { 
                    std::mem::transmute(set_thread_description) 
                };
                
                unsafe {
                    let _ = func(
                        winapi::um::processthreadsapi::GetCurrentThread(),
                        wide.as_ptr(),
                    );
                }
            }
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::os::raw::c_char;
        
        extern "C" {
            fn pthread_setname_np(thread: libc::pthread_t, name: *const c_char) -> libc::c_int;
        }
        
        let name_c = match CString::new(name) {
            Ok(s) => s,
            Err(_) => return, // Invalid name, skip
        };
        
        unsafe {
            let _ = pthread_setname_np(libc::pthread_self(), name_c.as_ptr());
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::raw::c_char;
        
        extern "C" {
            fn pthread_setname_np(name: *const c_char) -> libc::c_int;
        }
        
        let name_c = match CString::new(name) {
            Ok(s) => s,
            Err(_) => return, // Invalid name, skip
        };
        
        unsafe {
            let _ = pthread_setname_np(name_c.as_ptr());
        }
    }
    
    // For other platforms, thread naming may not be supported
    // This is a no-op but doesn't cause errors
}

/// Get the name of the current thread
/// Returns the name set by set_current_thread_name, or None if not set
pub fn get_current_thread_name() -> Option<String> {
    THREAD_NAME.with(|tn| tn.borrow().clone())
}

