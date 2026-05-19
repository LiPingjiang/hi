//! Input Method Engine (IME) control for macOS.
//!
//! Switches the system input source to ASCII (e.g. "ABC") on startup so that
//! Normal-mode keybindings work immediately without the user having to manually
//! dismiss a CJK input method.
//!
//! On non-macOS platforms this module is a no-op.

/// Attempt to switch the current input source to an ASCII-capable layout.
/// Silently does nothing on failure or on non-macOS systems.
pub fn switch_to_ascii() {
    #[cfg(target_os = "macos")]
    macos::switch_to_ascii();
}

#[cfg(target_os = "macos")]
mod macos {
    //! Direct FFI to macOS Carbon Text Input Source Services.
    //!
    //! We call:
    //!   - TISCopyCurrentKeyboardInputSource()
    //!   - TISGetInputSourceProperty(source, kTISPropertyInputSourceID)
    //!   - TISCopyInputSourceForLanguage(CFSTR("en"))
    //!   - TISSelectInputSource(source)
    //!
    //! Link against Carbon.framework (done via #[link]).

    use std::ptr;

    // Opaque types
    #[allow(non_camel_case_types)]
    type CFStringRef = *const std::ffi::c_void;
    #[allow(non_camel_case_types)]
    type CFArrayRef = *const std::ffi::c_void;
    #[allow(non_camel_case_types)]
    type TISInputSourceRef = *const std::ffi::c_void;
    #[allow(non_camel_case_types)]
    type CFIndex = isize;
    #[allow(non_camel_case_types)]
    type OSStatus = i32;
    #[allow(non_camel_case_types)]
    type CFDictionaryRef = *const std::ffi::c_void;
    #[allow(non_camel_case_types)]
    type CFBooleanRef = *const std::ffi::c_void;

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
        fn TISGetInputSourceProperty(
            source: TISInputSourceRef,
            key: CFStringRef,
        ) -> *const std::ffi::c_void;
        fn TISSelectInputSource(source: TISInputSourceRef) -> OSStatus;

        // For filtering input sources
        fn TISCreateInputSourceList(
            properties: CFDictionaryRef,
            include_all: u8,
        ) -> CFArrayRef;

        static kTISPropertyInputSourceID: CFStringRef;
        static kTISPropertyInputSourceCategory: CFStringRef;
        static kTISCategoryKeyboardInputSource: CFStringRef;
        static kTISPropertyInputSourceIsASCIICapable: CFStringRef;
        static kTISPropertyInputSourceIsSelectCapable: CFStringRef;
        static kTISPropertyInputSourceIsEnabled: CFStringRef;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const std::ffi::c_void,
            c_str: *const u8,
            encoding: u32,
        ) -> CFStringRef;
        fn CFRelease(cf: *const std::ffi::c_void);
        fn CFArrayGetCount(array: CFArrayRef) -> CFIndex;
        fn CFArrayGetValueAtIndex(
            array: CFArrayRef,
            idx: CFIndex,
        ) -> *const std::ffi::c_void;
        fn CFDictionaryCreate(
            allocator: *const std::ffi::c_void,
            keys: *const *const std::ffi::c_void,
            values: *const *const std::ffi::c_void,
            num_values: CFIndex,
            key_callbacks: *const std::ffi::c_void,
            value_callbacks: *const std::ffi::c_void,
        ) -> CFDictionaryRef;
        fn CFBooleanGetValue(boolean: CFBooleanRef) -> u8;

        static kCFBooleanTrue: CFBooleanRef;
        static kCFTypeDictionaryKeyCallBacks: std::ffi::c_void;
        static kCFTypeDictionaryValueCallBacks: std::ffi::c_void;
    }

    /// Check if the current input source is already ASCII-capable.
    unsafe fn current_is_ascii() -> bool {
        let current = TISCopyCurrentKeyboardInputSource();
        if current.is_null() {
            return true; // assume ASCII if we can't determine
        }
        let ascii_prop = TISGetInputSourceProperty(
            current,
            kTISPropertyInputSourceIsASCIICapable,
        );
        CFRelease(current);
        if ascii_prop.is_null() {
            return true;
        }
        // The property value is a CFBoolean
        CFBooleanGetValue(ascii_prop as CFBooleanRef) != 0
    }

    /// Find the first ASCII-capable, enabled, selectable keyboard input source
    /// and select it.
    unsafe fn select_first_ascii_source() -> bool {
        // Build a filter dictionary:
        //   kTISPropertyInputSourceCategory = kTISCategoryKeyboardInputSource
        //   kTISPropertyInputSourceIsASCIICapable = kCFBooleanTrue
        //   kTISPropertyInputSourceIsSelectCapable = kCFBooleanTrue
        //   kTISPropertyInputSourceIsEnabled = kCFBooleanTrue
        let keys: [*const std::ffi::c_void; 4] = [
            kTISPropertyInputSourceCategory as *const _,
            kTISPropertyInputSourceIsASCIICapable as *const _,
            kTISPropertyInputSourceIsSelectCapable as *const _,
            kTISPropertyInputSourceIsEnabled as *const _,
        ];
        let values: [*const std::ffi::c_void; 4] = [
            kTISCategoryKeyboardInputSource as *const _,
            kCFBooleanTrue as *const _,
            kCFBooleanTrue as *const _,
            kCFBooleanTrue as *const _,
        ];

        let dict = CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            4,
            &kCFTypeDictionaryKeyCallBacks as *const _ as *const _,
            &kCFTypeDictionaryValueCallBacks as *const _ as *const _,
        );
        if dict.is_null() {
            return false;
        }

        let sources = TISCreateInputSourceList(dict, 0);
        CFRelease(dict);
        if sources.is_null() {
            return false;
        }

        let count = CFArrayGetCount(sources);
        if count <= 0 {
            CFRelease(sources);
            return false;
        }

        // Prefer "com.apple.keylayout.ABC" or "com.apple.keylayout.US"
        let preferred = [
            "com.apple.keylayout.ABC\0",
            "com.apple.keylayout.US\0",
        ];

        // First pass: look for preferred sources
        for pref in &preferred {
            let pref_cf = CFStringCreateWithCString(
                ptr::null(),
                pref.as_ptr(),
                K_CF_STRING_ENCODING_UTF8,
            );
            if pref_cf.is_null() {
                continue;
            }
            for i in 0..count {
                let source = CFArrayGetValueAtIndex(sources, i) as TISInputSourceRef;
                let source_id = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
                if !source_id.is_null() && cf_string_eq(source_id as CFStringRef, pref_cf) {
                    CFRelease(pref_cf);
                    let status = TISSelectInputSource(source);
                    CFRelease(sources);
                    return status == 0;
                }
            }
            CFRelease(pref_cf);
        }

        // Fallback: select the first ASCII-capable source
        let source = CFArrayGetValueAtIndex(sources, 0) as TISInputSourceRef;
        let status = TISSelectInputSource(source);
        CFRelease(sources);
        status == 0
    }

    /// Compare two CFStringRef values for equality.
    unsafe fn cf_string_eq(a: CFStringRef, b: CFStringRef) -> bool {
        // Use CFStringCompare
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFStringCompare(
                a: CFStringRef,
                b: CFStringRef,
                options: u64,
            ) -> i64;
        }
        CFStringCompare(a, b, 0) == 0
    }

    pub fn switch_to_ascii() {
        unsafe {
            if !current_is_ascii() {
                select_first_ascii_source();
            }
        }
    }
}
