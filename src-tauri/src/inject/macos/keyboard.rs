//! Posting Cmd+V for the active keyboard layout.
//!
//! The layout lookup is adapted from Handy (https://github.com/cjpais/Handy,
//! src-tauri/src/input.rs), MIT License, Copyright (c) CJ Pais.

use std::ffi::c_void;

use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
};

type TisInputSourceRef = *const c_void;
type CfDataRef = *const c_void;
type CfStringRef = *const c_void;

/// kVK_ANSI_V: the physical V key on a US layout. Fallback when the active
/// layout cannot be read.
const ANSI_V_KEYCODE: u16 = 9;
const KEYCODE_COUNT: u16 = 128;
const UC_KEY_ACTION_DISPLAY: u16 = 3;
const UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK: u32 = 1;
/// Carbon's cmdKey (bit 8) shifted right by 8, as UCKeyTranslate expects.
const COMMAND_MODIFIER_STATE: u32 = 1;

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCopyCurrentKeyboardLayoutInputSource() -> TisInputSourceRef;
    fn TISGetInputSourceProperty(source: TisInputSourceRef, key: CfStringRef) -> CfDataRef;
    static kTISPropertyUnicodeKeyLayoutData: CfStringRef;
    #[allow(clippy::too_many_arguments)]
    fn UCKeyTranslate(
        key_layout: *const u8,
        virtual_key_code: u16,
        key_action: u16,
        modifier_key_state: u32,
        keyboard_type: u32,
        key_translate_options: u32,
        dead_key_state: *mut u32,
        max_string_length: usize,
        actual_string_length: *mut usize,
        unicode_string: *mut u16,
    ) -> i32;
    fn LMGetKbdType() -> u8;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFDataGetBytePtr(data: CfDataRef) -> *const u8;
    fn CFRelease(value: *const c_void);
}

/// Owns a retained TIS input source and releases it on drop.
struct InputSource(TisInputSourceRef);

impl Drop for InputSource {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: TISCopyCurrentKeyboardLayoutInputSource returned a +1 reference.
            unsafe { CFRelease(self.0) };
        }
    }
}

/// Finds the key that types "v" while Command is held in the current layout
/// (differs on e.g. Dvorak). Must run on the main thread (TIS APIs).
fn command_v_keycode() -> u16 {
    match resolve_command_v_keycode() {
        Ok(code) => code,
        Err(error) => {
            log::warn!("{error}; falling back to ANSI V");
            ANSI_V_KEYCODE
        }
    }
}

fn resolve_command_v_keycode() -> Result<u16, String> {
    // SAFETY: called on the main thread; the source is released by InputSource::drop.
    let source = InputSource(unsafe { TISCopyCurrentKeyboardLayoutInputSource() });
    if source.0.is_null() {
        return Err("no current keyboard layout".into());
    }
    // SAFETY: the source is retained for the whole scan; the key is a Carbon constant.
    let layout_data =
        unsafe { TISGetInputSourceProperty(source.0, kTISPropertyUnicodeKeyLayoutData) };
    if layout_data.is_null() {
        return Err("keyboard layout has no unicode data".into());
    }
    // SAFETY: layout_data is owned by the retained source.
    let layout = unsafe { CFDataGetBytePtr(layout_data) };
    if layout.is_null() {
        return Err("keyboard layout data is empty".into());
    }
    // SAFETY: no arguments; returns the physical keyboard type.
    let keyboard_type = unsafe { LMGetKbdType() } as u32;

    (0..KEYCODE_COUNT)
        .find(|&keycode| {
            let mut dead_key_state = 0_u32;
            let mut chars = [0_u16; 4];
            let mut length = 0_usize;
            // SAFETY: `layout` stays valid while `source` lives; all out-pointers
            // point to initialised locals of the declared sizes.
            let status = unsafe {
                UCKeyTranslate(
                    layout,
                    keycode,
                    UC_KEY_ACTION_DISPLAY,
                    COMMAND_MODIFIER_STATE,
                    keyboard_type,
                    UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK,
                    &mut dead_key_state,
                    chars.len(),
                    &mut length,
                    chars.as_mut_ptr(),
                )
            };
            status == 0 && length == 1 && chars[0] == u16::from(b'v')
        })
        .ok_or_else(|| "no key maps to Cmd+V in this layout".into())
}

/// Posts Cmd+V (key down + key up). Requires the Accessibility permission.
pub fn post_command_v() -> Result<(), String> {
    let keycode = command_v_keycode();
    // A private event source: modifier keys the user is physically holding
    // (e.g. a hotkey modifier) do not leak into the synthetic event.
    let source = CGEventSource::new(CGEventSourceStateID::Private);
    for key_down in [true, false] {
        let event = CGEvent::new_keyboard_event(source.as_deref(), keycode, key_down)
            .ok_or("could not create keyboard event")?;
        CGEvent::set_flags(Some(&event), CGEventFlags::MaskCommand);
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
    }
    Ok(())
}
