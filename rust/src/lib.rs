//! The RetrOS SDK for Rust apps (plans.md §5.1).
//!
//! Apps compile to `wasm32-unknown-unknown` and are loaded by the engine's `wasmtime`
//! host. Everything below is a safe wrapper over the raw host imports in [`ffi`]; the
//! wrapper surface mirrors §5's table exactly, so the same program shape works in Rust,
//! Python and Lua.
//!
//! ```ignore
//! #[retros::main]
//! fn tick(_dt: f32) {
//!     retros::clear(retros::rgb(0, 0, 40));
//!     retros::draw_text(4, 4, "Hello, RetrOS", retros::rgb(255, 255, 255));
//! }
//! ```
//!
//! # The ABI in one paragraph
//!
//! wasm has no strings, so text crosses the boundary as `(ptr, len)` pairs into the
//! app's own linear memory. Values coming *back* are written into a buffer the app
//! supplies: the host returns the number of bytes written, or — if the buffer was too
//! small — the number of bytes needed, and the app retries. That avoids having the host
//! allocate inside the guest, which would mean calling back into guest exports mid-call.

#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

pub use retros_sdk_macros::main;

/// Raw host imports. Prefer the safe wrappers; these are public so unusual apps are not
/// boxed in by the wrapper's choices.
pub mod ffi {
    #[link(wasm_import_module = "retros")]
    unsafe extern "C" {
        // input
        pub fn key_down(ptr: *const u8, len: i32) -> i32;
        pub fn key_pressed(ptr: *const u8, len: i32) -> i32;
        pub fn key_released(ptr: *const u8, len: i32) -> i32;
        pub fn text_input(out: *mut u8, cap: i32) -> i32;
        pub fn mouse_x() -> i32;
        pub fn mouse_y() -> i32;
        pub fn mouse_button_down(ptr: *const u8, len: i32) -> i32;
        pub fn mouse_button_pressed(ptr: *const u8, len: i32) -> i32;
        pub fn mouse_button_released(ptr: *const u8, len: i32) -> i32;
        pub fn mouse_wheel() -> f64;

        // time
        pub fn now_unix() -> i64;
        pub fn now_iso8601(out: *mut u8, cap: i32) -> i32;
        pub fn uptime_ms() -> i64;
        pub fn frame_count() -> i64;
        pub fn delta_time() -> f64;
        pub fn sleep_frames(frames: i64);

        // random
        pub fn rand_seed(seed: i64);
        pub fn rand_int(min: i64, max: i64) -> i64;
        pub fn rand_float() -> f64;

        // display
        pub fn clear(color: i32);
        pub fn draw_pixel(x: i32, y: i32, color: i32);
        pub fn draw_rect(x: i32, y: i32, w: i32, h: i32, color: i32, filled: i32);
        pub fn draw_line(x0: i32, y0: i32, x1: i32, y1: i32, color: i32);
        pub fn draw_text(x: i32, y: i32, ptr: *const u8, len: i32, color: i32);
        pub fn draw_sprite(x: i32, y: i32, sprite: i32);
        pub fn load_sprite(ptr: *const u8, len: i32) -> i32;
        pub fn sprite_width(sprite: i32) -> i32;
        pub fn sprite_height(sprite: i32) -> i32;
        pub fn text_width(ptr: *const u8, len: i32) -> i32;
        pub fn text_height(ptr: *const u8, len: i32) -> i32;
        pub fn screen_width() -> i32;
        pub fn screen_height() -> i32;
        pub fn window_width() -> i32;
        pub fn window_height() -> i32;
        pub fn window_id() -> i32;

        // window
        pub fn set_title(ptr: *const u8, len: i32);
        pub fn resize(w: i32, h: i32);
        pub fn request_focus();
        pub fn close();
        pub fn move_window(x: i32, y: i32);
        pub fn set_window_state(ptr: *const u8, len: i32) -> i32;
        pub fn window_state(out: *mut u8, cap: i32) -> i32;
        pub fn work_area_x() -> i32;
        pub fn work_area_y() -> i32;
        pub fn work_area_w() -> i32;
        pub fn work_area_h() -> i32;

        // direct to screen (needs permissions.direct_screen)
        pub fn screen_pixel(x: i32, y: i32, color: i32) -> i32;
        pub fn screen_rect(x: i32, y: i32, w: i32, h: i32, color: i32, filled: i32) -> i32;
        pub fn screen_line(x0: i32, y0: i32, x1: i32, y1: i32, color: i32) -> i32;
        pub fn screen_text(x: i32, y: i32, ptr: *const u8, len: i32, color: i32) -> i32;
        pub fn screen_sprite(sprite: i32, x: i32, y: i32) -> i32;

        // filesystem
        pub fn fs_read(path: *const u8, path_len: i32, out: *mut u8, cap: i32) -> i32;
        pub fn fs_write(path: *const u8, path_len: i32, data: *const u8, data_len: i32) -> i32;
        pub fn fs_list(dir: *const u8, dir_len: i32, out: *mut u8, cap: i32) -> i32;
        pub fn fs_delete(path: *const u8, path_len: i32) -> i32;
        pub fn fs_exists(path: *const u8, path_len: i32) -> i32;
        pub fn fs_mkdir(path: *const u8, path_len: i32) -> i32;
        pub fn fs_rename(from: *const u8, from_len: i32, to: *const u8, to_len: i32) -> i32;
        pub fn fs_copy(from: *const u8, from_len: i32, to: *const u8, to_len: i32) -> i32;
        pub fn fs_stat(path: *const u8, path_len: i32, out: *mut u8, cap: i32) -> i32;

        // memory
        pub fn mem_get(key: *const u8, key_len: i32, out: *mut u8, cap: i32) -> i32;
        pub fn mem_set(key: *const u8, key_len: i32, value: *const u8, value_len: i32) -> i32;
        pub fn mem_get_shared(key: *const u8, key_len: i32, out: *mut u8, cap: i32) -> i32;
        pub fn mem_set_shared(key: *const u8, key_len: i32, value: *const u8, value_len: i32) -> i32;

        // system
        pub fn list_packages(out: *mut u8, cap: i32) -> i32;
        pub fn launch_package(id: *const u8, id_len: i32, args: *const u8, args_len: i32) -> i32;
        pub fn rospm(argv: *const u8, argv_len: i32, out: *mut u8, cap: i32) -> i32;
        pub fn args(out: *mut u8, cap: i32) -> i32;
        pub fn package_id(out: *mut u8, cap: i32) -> i32;
        pub fn exit();
        pub fn log(ptr: *const u8, len: i32);

        // settings
        pub fn setting(key: *const u8, key_len: i32, out: *mut u8, cap: i32) -> i32;
        pub fn setting_set(
            key: *const u8,
            key_len: i32,
            value: *const u8,
            value_len: i32,
            out: *mut u8,
            cap: i32,
        ) -> i32;
        pub fn settings_get(
            package: *const u8,
            package_len: i32,
            key: *const u8,
            key_len: i32,
            out: *mut u8,
            cap: i32,
        ) -> i32;
        pub fn settings_set(
            package: *const u8,
            package_len: i32,
            key: *const u8,
            key_len: i32,
            value: *const u8,
            value_len: i32,
            out: *mut u8,
            cap: i32,
        ) -> i32;
        pub fn settings_pages(out: *mut u8, cap: i32) -> i32;
        pub fn settings_options(
            package: *const u8,
            package_len: i32,
            out: *mut u8,
            cap: i32,
        ) -> i32;

        // start menu
        pub fn menu_entries(out: *mut u8, cap: i32) -> i32;
        pub fn launch_entry(id: *const u8, id_len: i32) -> i32;

        // window management (needs permissions.window_manager)
        pub fn win_list(out: *mut u8, cap: i32) -> i32;
        pub fn win_title(window: i32, out: *mut u8, cap: i32) -> i32;
        pub fn win_package(window: i32, out: *mut u8, cap: i32) -> i32;
        pub fn win_move(window: i32, x: i32, y: i32) -> i32;
        pub fn win_resize(window: i32, w: i32, h: i32) -> i32;
        pub fn win_raise(window: i32) -> i32;
        pub fn win_lower(window: i32) -> i32;
        pub fn win_focus(window: i32) -> i32;
        pub fn win_set_visible(window: i32, visible: i32) -> i32;
        pub fn win_close(window: i32) -> i32;
        pub fn win_focused() -> i32;
        pub fn win_at(x: i32, y: i32) -> i32;
        pub fn win_set_state(window: i32, ptr: *const u8, len: i32) -> i32;
        pub fn win_state(window: i32, out: *mut u8, cap: i32) -> i32;
        pub fn win_hit_test(x: i32, y: i32, out: *mut u8, cap: i32) -> i32;
        pub fn win_set_work_area(x: i32, y: i32, w: i32, h: i32) -> i32;
        pub fn set_cursor(ptr: *const u8, len: i32) -> i32;
        pub fn chrome_title_height() -> i32;
        pub fn chrome_border_width() -> i32;
    }
}

// ------------------------------------------------------------------ helpers --

/// Most strings the host returns are short; start small and only grow if told to.
const SMALL: usize = 256;

/// Call a host function that fills a caller-supplied buffer, growing once if needed.
fn read_string(mut call: impl FnMut(*mut u8, i32) -> i32) -> Option<String> {
    let mut buffer = vec![0u8; SMALL];
    let written = call(buffer.as_mut_ptr(), buffer.len() as i32);
    if written < 0 {
        return None;
    }
    let mut written = written as usize;
    if written > buffer.len() {
        // The host reported how much room it actually needs.
        buffer = vec![0u8; written];
        let again = call(buffer.as_mut_ptr(), buffer.len() as i32);
        if again < 0 {
            return None;
        }
        written = (again as usize).min(buffer.len());
    }
    buffer.truncate(written);
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

fn read_bytes(mut call: impl FnMut(*mut u8, i32) -> i32) -> Option<Vec<u8>> {
    let mut buffer = vec![0u8; SMALL];
    let written = call(buffer.as_mut_ptr(), buffer.len() as i32);
    if written < 0 {
        return None;
    }
    let mut written = written as usize;
    if written > buffer.len() {
        buffer = vec![0u8; written];
        let again = call(buffer.as_mut_ptr(), buffer.len() as i32);
        if again < 0 {
            return None;
        }
        written = (again as usize).min(buffer.len());
    }
    buffer.truncate(written);
    Some(buffer)
}

/// Pack channels into the engine's `0xAARRGGBB` color word.
pub const fn rgb(r: u8, g: u8, b: u8) -> i32 {
    rgba(r, g, b, 255)
}

pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> i32 {
    (((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32) as i32
}

// -------------------------------------------------------------------- input --

pub fn key_down(key: &str) -> bool {
    unsafe { ffi::key_down(key.as_ptr(), key.len() as i32) != 0 }
}

pub fn key_pressed(key: &str) -> bool {
    unsafe { ffi::key_pressed(key.as_ptr(), key.len() as i32) != 0 }
}

pub fn key_released(key: &str) -> bool {
    unsafe { ffi::key_released(key.as_ptr(), key.len() as i32) != 0 }
}

/// Characters typed this frame, in order.
pub fn text_input() -> String {
    read_string(|out, cap| unsafe { ffi::text_input(out, cap) }).unwrap_or_default()
}

/// Pointer position in this app's own window coordinates.
pub fn mouse_pos() -> (i32, i32) {
    unsafe { (ffi::mouse_x(), ffi::mouse_y()) }
}

pub fn mouse_button_down(button: &str) -> bool {
    unsafe { ffi::mouse_button_down(button.as_ptr(), button.len() as i32) != 0 }
}

pub fn mouse_button_pressed(button: &str) -> bool {
    unsafe { ffi::mouse_button_pressed(button.as_ptr(), button.len() as i32) != 0 }
}

pub fn mouse_button_released(button: &str) -> bool {
    unsafe { ffi::mouse_button_released(button.as_ptr(), button.len() as i32) != 0 }
}

pub fn mouse_wheel() -> f64 {
    unsafe { ffi::mouse_wheel() }
}

// --------------------------------------------------------------------- time --

pub fn now_unix() -> u64 {
    unsafe { ffi::now_unix() as u64 }
}

pub fn now_iso8601() -> String {
    read_string(|out, cap| unsafe { ffi::now_iso8601(out, cap) }).unwrap_or_default()
}

pub fn uptime_ms() -> u64 {
    unsafe { ffi::uptime_ms() as u64 }
}

pub fn frame_count() -> u64 {
    unsafe { ffi::frame_count() as u64 }
}

pub fn delta_time() -> f64 {
    unsafe { ffi::delta_time() }
}

/// Skip the next `frames` ticks.
///
/// This is RetrOS's only form of sleeping, in every language. A blocking sleep would
/// stall the single shared frame loop and freeze the entire desktop, not just this app.
pub fn sleep_frames(frames: u64) {
    unsafe { ffi::sleep_frames(frames as i64) }
}

// ------------------------------------------------------------------- random --

pub fn rand_seed(seed: u64) {
    unsafe { ffi::rand_seed(seed as i64) }
}

pub fn rand_int(min: i64, max: i64) -> i64 {
    unsafe { ffi::rand_int(min, max) }
}

pub fn rand_float() -> f64 {
    unsafe { ffi::rand_float() }
}

// ------------------------------------------------------------------ display --

pub fn clear(color: i32) {
    unsafe { ffi::clear(color) }
}

pub fn draw_pixel(x: i32, y: i32, color: i32) {
    unsafe { ffi::draw_pixel(x, y, color) }
}

pub fn draw_rect(x: i32, y: i32, w: i32, h: i32, color: i32, filled: bool) {
    unsafe { ffi::draw_rect(x, y, w, h, color, filled as i32) }
}

pub fn draw_line(x0: i32, y0: i32, x1: i32, y1: i32, color: i32) {
    unsafe { ffi::draw_line(x0, y0, x1, y1, color) }
}

pub fn draw_text(x: i32, y: i32, text: &str, color: i32) {
    unsafe { ffi::draw_text(x, y, text.as_ptr(), text.len() as i32, color) }
}

pub fn draw_sprite(x: i32, y: i32, sprite: i32) {
    unsafe { ffi::draw_sprite(x, y, sprite) }
}

/// Load a PNG from the package's own directory. Returns `None` if it could not be read.
pub fn load_sprite(path: &str) -> Option<i32> {
    let id = unsafe { ffi::load_sprite(path.as_ptr(), path.len() as i32) };
    (id >= 0).then_some(id)
}

pub fn sprite_size(sprite: i32) -> (i32, i32) {
    unsafe { (ffi::sprite_width(sprite), ffi::sprite_height(sprite)) }
}

pub fn text_size(text: &str) -> (i32, i32) {
    unsafe {
        (
            ffi::text_width(text.as_ptr(), text.len() as i32),
            ffi::text_height(text.as_ptr(), text.len() as i32),
        )
    }
}

pub fn screen_size() -> (i32, i32) {
    unsafe { (ffi::screen_width(), ffi::screen_height()) }
}

pub fn window_size() -> (i32, i32) {
    unsafe { (ffi::window_width(), ffi::window_height()) }
}

/// This app's own window id, or `0` if it has none. A package that also uses the
/// window-management calls needs this to tell its own window from the ones it manages.
pub fn window_id() -> i32 {
    unsafe { ffi::window_id() }
}

// ------------------------------------------------------------------- window --

pub fn set_title(title: &str) {
    unsafe { ffi::set_title(title.as_ptr(), title.len() as i32) }
}

pub fn resize(w: i32, h: i32) {
    unsafe { ffi::resize(w, h) }
}

pub fn request_focus() {
    unsafe { ffi::request_focus() }
}

pub fn close() {
    unsafe { ffi::close() }
}

/// Move this app's own window. Needs no permission: it is the app's window.
pub fn move_window(x: i32, y: i32) {
    unsafe { ffi::move_window(x, y) }
}

/// `"normal"`, `"minimized"` or `"maximized"`. Returns `false` if the window refuses.
pub fn set_window_state(state: &str) -> bool {
    unsafe { ffi::set_window_state(state.as_ptr(), state.len() as i32) == 0 }
}

pub fn window_state() -> String {
    read_string(|out, cap| unsafe { ffi::window_state(out, cap) }).unwrap_or_default()
}

pub fn minimize() -> bool {
    set_window_state("minimized")
}

pub fn maximize() -> bool {
    set_window_state("maximized")
}

pub fn restore() -> bool {
    set_window_state("normal")
}

/// `(x, y, w, h)` of the screen area left free by panels, so an app can place itself
/// without needing a privileged permission to find out where the taskbar is.
pub fn work_area() -> (i32, i32, i32, i32) {
    unsafe { (ffi::work_area_x(), ffi::work_area_y(), ffi::work_area_w(), ffi::work_area_h()) }
}

// ---------------------------------------------------------- direct to screen --
//
// Painting outside any window, for wallpapers, on-screen displays and a window manager's
// drag outline. Every call needs `permissions.direct_screen` and returns `false` without
// it. The drawing lands on top of the composited frame, under the cursor.

pub fn screen_pixel(x: i32, y: i32, color: i32) -> bool {
    unsafe { ffi::screen_pixel(x, y, color) == 0 }
}

pub fn screen_rect(x: i32, y: i32, w: i32, h: i32, color: i32, filled: bool) -> bool {
    unsafe { ffi::screen_rect(x, y, w, h, color, filled as i32) == 0 }
}

pub fn screen_line(x0: i32, y0: i32, x1: i32, y1: i32, color: i32) -> bool {
    unsafe { ffi::screen_line(x0, y0, x1, y1, color) == 0 }
}

pub fn screen_text(x: i32, y: i32, text: &str, color: i32) -> bool {
    unsafe { ffi::screen_text(x, y, text.as_ptr(), text.len() as i32, color) == 0 }
}

pub fn screen_sprite(sprite: i32, x: i32, y: i32) -> bool {
    unsafe { ffi::screen_sprite(sprite, x, y) == 0 }
}

// --------------------------------------------------------------- filesystem --

pub fn fs_read(path: &str) -> Option<Vec<u8>> {
    read_bytes(|out, cap| unsafe { ffi::fs_read(path.as_ptr(), path.len() as i32, out, cap) })
}

pub fn fs_read_string(path: &str) -> Option<String> {
    fs_read(path).map(|b| String::from_utf8_lossy(&b).into_owned())
}

pub fn fs_write(path: &str, data: &[u8]) -> bool {
    unsafe {
        ffi::fs_write(path.as_ptr(), path.len() as i32, data.as_ptr(), data.len() as i32) == 0
    }
}

/// One entry in a directory listing.
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    /// Seconds since the Unix epoch, or zero when the platform would not say.
    pub modified: u64,
}

pub fn fs_list(dir: &str) -> Vec<Entry> {
    let raw = read_string(|out, cap| unsafe {
        ffi::fs_list(dir.as_ptr(), dir.len() as i32, out, cap)
    })
    .unwrap_or_default();

    raw.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            // "name\tis_dir\tsize\tmodified"
            let mut parts = line.split('\t');
            let name = parts.next()?;
            let is_dir = parts.next()? == "1";
            let size = parts.next()?.parse().ok()?;
            let modified = parts.next()?.parse().ok()?;
            Some(Entry { name: String::from(name), is_dir, size, modified })
        })
        .collect()
}

pub fn fs_delete(path: &str) -> bool {
    unsafe { ffi::fs_delete(path.as_ptr(), path.len() as i32) == 0 }
}

pub fn fs_exists(path: &str) -> bool {
    unsafe { ffi::fs_exists(path.as_ptr(), path.len() as i32) != 0 }
}

pub fn fs_mkdir(path: &str) -> bool {
    unsafe { ffi::fs_mkdir(path.as_ptr(), path.len() as i32) == 0 }
}

/// Move or rename inside the sandbox. Both ends are resolved, so this can neither push a
/// file out of the sandbox nor pull one in.
pub fn fs_rename(from: &str, to: &str) -> bool {
    unsafe {
        ffi::fs_rename(from.as_ptr(), from.len() as i32, to.as_ptr(), to.len() as i32) == 0
    }
}

pub fn fs_copy(from: &str, to: &str) -> bool {
    unsafe { ffi::fs_copy(from.as_ptr(), from.len() as i32, to.as_ptr(), to.len() as i32) == 0 }
}

/// Metadata for one path. Like [`Entry`] but with the modification time, which a listing
/// does not carry.
pub struct Stat {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
}

/// `None` when nothing is at `path`, so "does it exist, and if so what is it" is one call.
pub fn fs_stat(path: &str) -> Option<Stat> {
    let raw =
        read_string(|out, cap| unsafe { ffi::fs_stat(path.as_ptr(), path.len() as i32, out, cap) })?;
    // "name\tis_dir\tsize\tmodified"
    let mut parts = raw.split('\t');
    Some(Stat {
        name: String::from(parts.next()?),
        is_dir: parts.next()? == "1",
        size: parts.next()?.parse().ok()?,
        modified: parts.next()?.parse().ok()?,
    })
}

// ------------------------------------------------------------------- memory --

pub fn mem_get(key: &str) -> Option<String> {
    read_string(|out, cap| unsafe { ffi::mem_get(key.as_ptr(), key.len() as i32, out, cap) })
}

pub fn mem_set(key: &str, value: &str) -> bool {
    unsafe {
        ffi::mem_set(key.as_ptr(), key.len() as i32, value.as_ptr(), value.len() as i32) == 0
    }
}

pub fn mem_get_shared(key: &str) -> Option<String> {
    read_string(|out, cap| unsafe {
        ffi::mem_get_shared(key.as_ptr(), key.len() as i32, out, cap)
    })
}

/// Requires `permissions.shared_mem_write`; returns `false` if it was not granted.
pub fn mem_set_shared(key: &str, value: &str) -> bool {
    unsafe {
        ffi::mem_set_shared(key.as_ptr(), key.len() as i32, value.as_ptr(), value.len() as i32)
            == 0
    }
}

// ------------------------------------------------------------------- system --

pub struct PackageInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub core: bool,
    pub running: bool,
}

pub fn list_packages() -> Vec<PackageInfo> {
    let raw = read_string(|out, cap| unsafe { ffi::list_packages(out, cap) }).unwrap_or_default();
    raw.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(PackageInfo {
                id: String::from(parts.next()?),
                name: String::from(parts.next()?),
                version: String::from(parts.next()?),
                kind: String::from(parts.next()?),
                core: parts.next()? == "1",
                running: parts.next()? == "1",
            })
        })
        .collect()
}

pub fn launch_package(id: &str, args: &str) -> bool {
    unsafe {
        ffi::launch_package(id.as_ptr(), id.len() as i32, args.as_ptr(), args.len() as i32) == 0
    }
}

/// Run `rospm` and return its output. Requires `permissions.package_manager`, which
/// only the `console` package declares by default.
pub fn rospm(args: &[&str]) -> Option<String> {
    let joined = args.join("\n");
    read_string(|out, cap| unsafe {
        ffi::rospm(joined.as_ptr(), joined.len() as i32, out, cap)
    })
}

/// The arguments this app was launched with.
pub fn args() -> String {
    read_string(|out, cap| unsafe { ffi::args(out, cap) }).unwrap_or_default()
}

pub fn package_id() -> String {
    read_string(|out, cap| unsafe { ffi::package_id(out, cap) }).unwrap_or_default()
}

/// Shut down the whole system.
pub fn exit() {
    unsafe { ffi::exit() }
}

pub fn log(message: &str) {
    unsafe { ffi::log(message.as_ptr(), message.len() as i32) }
}

// ----------------------------------------------------------------- settings --
//
// A package declares its settings schema in package.toml and reads or writes its own
// values freely. Touching another package's settings needs `permissions.settings_write`.

/// One of this package's own settings, as stored.
pub fn setting(key: &str) -> Option<String> {
    read_string(|out, cap| unsafe { ffi::setting(key.as_ptr(), key.len() as i32, out, cap) })
}

/// Returns the value as stored, which the schema may have clamped or normalised.
pub fn setting_set(key: &str, value: &str) -> Option<String> {
    read_string(|out, cap| unsafe {
        ffi::setting_set(
            key.as_ptr(),
            key.len() as i32,
            value.as_ptr(),
            value.len() as i32,
            out,
            cap,
        )
    })
}

pub fn settings_get(package: &str, key: &str) -> Option<String> {
    read_string(|out, cap| unsafe {
        ffi::settings_get(
            package.as_ptr(),
            package.len() as i32,
            key.as_ptr(),
            key.len() as i32,
            out,
            cap,
        )
    })
}

pub fn settings_set(package: &str, key: &str, value: &str) -> Option<String> {
    read_string(|out, cap| unsafe {
        ffi::settings_set(
            package.as_ptr(),
            package.len() as i32,
            key.as_ptr(),
            key.len() as i32,
            value.as_ptr(),
            value.len() as i32,
            out,
            cap,
        )
    })
}

/// One settings page, contributed by one package.
pub struct SettingsPage {
    pub package: String,
    pub title: String,
    pub category: String,
    pub parent: String,
    pub option_count: u32,
}

pub fn settings_pages() -> Vec<SettingsPage> {
    let raw = read_string(|out, cap| unsafe { ffi::settings_pages(out, cap) }).unwrap_or_default();
    raw.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(SettingsPage {
                package: String::from(parts.next()?),
                title: String::from(parts.next()?),
                category: String::from(parts.next()?),
                parent: String::from(parts.next()?),
                option_count: parts.next()?.parse().ok()?,
            })
        })
        .collect()
}

/// One control on a settings page, carrying its current value.
pub struct SettingsOption {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub value: String,
    pub default: String,
    pub help: String,
    pub min: i64,
    pub max: i64,
    pub choices: Vec<String>,
}

pub fn settings_options(package: &str) -> Vec<SettingsOption> {
    let raw = read_string(|out, cap| unsafe {
        ffi::settings_options(package.as_ptr(), package.len() as i32, out, cap)
    })
    .unwrap_or_default();

    raw.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let key = String::from(parts.next()?);
            let label = String::from(parts.next()?);
            let kind = String::from(parts.next()?);
            let value = String::from(parts.next()?);
            let default = String::from(parts.next()?);
            let help = String::from(parts.next()?);
            let min = parts.next()?.parse().ok()?;
            let max = parts.next()?.parse().ok()?;
            // A list nested in a row needs its own separator; the host uses U+001F.
            let choices = parts
                .next()?
                .split('\u{1f}')
                .filter(|c| !c.is_empty())
                .map(String::from)
                .collect();
            Some(SettingsOption { key, label, kind, value, default, help, min, max, choices })
        })
        .collect()
}

// --------------------------------------------------------------- start menu --

pub struct MenuEntry {
    pub id: String,
    pub name: String,
    pub comment: String,
    pub package: String,
    pub args: String,
    pub icon: String,
    pub category: String,
    pub subcategory: String,
}

pub fn menu_entries() -> Vec<MenuEntry> {
    let raw = read_string(|out, cap| unsafe { ffi::menu_entries(out, cap) }).unwrap_or_default();
    raw.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(MenuEntry {
                id: String::from(parts.next()?),
                name: String::from(parts.next()?),
                comment: String::from(parts.next()?),
                package: String::from(parts.next()?),
                args: String::from(parts.next()?),
                icon: String::from(parts.next()?),
                category: String::from(parts.next()?),
                subcategory: String::from(parts.next()?),
            })
        })
        .collect()
}

/// Launch what a menu entry points at, its arguments included.
pub fn launch_entry(id: &str) -> bool {
    unsafe { ffi::launch_entry(id.as_ptr(), id.len() as i32) == 0 }
}

// ------------------------------------------------------- window management --
//
// Every call here needs `permissions.window_manager = true` in package.toml.

pub struct WindowInfo {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub focused: bool,
    pub visible: bool,
    /// `"normal"`, `"minimized"` or `"maximized"`.
    pub state: String,
    pub minimizable: bool,
    pub maximizable: bool,
    pub always_on_top: bool,
    pub always_on_bottom: bool,
}

pub fn win_list() -> Vec<WindowInfo> {
    let raw = read_string(|out, cap| unsafe { ffi::win_list(out, cap) }).unwrap_or_default();
    raw.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(WindowInfo {
                id: parts.next()?.parse().ok()?,
                x: parts.next()?.parse().ok()?,
                y: parts.next()?.parse().ok()?,
                w: parts.next()?.parse().ok()?,
                h: parts.next()?.parse().ok()?,
                focused: parts.next()? == "1",
                visible: parts.next()? == "1",
                state: String::from(parts.next()?),
                minimizable: parts.next()? == "1",
                maximizable: parts.next()? == "1",
                always_on_top: parts.next()? == "1",
                always_on_bottom: parts.next()? == "1",
            })
        })
        .collect()
}

pub fn win_title(window: i32) -> String {
    read_string(|out, cap| unsafe { ffi::win_title(window, out, cap) }).unwrap_or_default()
}

pub fn win_package(window: i32) -> String {
    read_string(|out, cap| unsafe { ffi::win_package(window, out, cap) }).unwrap_or_default()
}

pub fn win_move(window: i32, x: i32, y: i32) -> bool {
    unsafe { ffi::win_move(window, x, y) == 0 }
}

pub fn win_resize(window: i32, w: i32, h: i32) -> bool {
    unsafe { ffi::win_resize(window, w, h) == 0 }
}

pub fn win_raise(window: i32) -> bool {
    unsafe { ffi::win_raise(window) == 0 }
}

pub fn win_lower(window: i32) -> bool {
    unsafe { ffi::win_lower(window) == 0 }
}

pub fn win_focus(window: i32) -> bool {
    unsafe { ffi::win_focus(window) == 0 }
}

pub fn win_set_visible(window: i32, visible: bool) -> bool {
    unsafe { ffi::win_set_visible(window, visible as i32) == 0 }
}

pub fn win_close(window: i32) -> bool {
    unsafe { ffi::win_close(window) == 0 }
}

/// The focused window id, or `0` when nothing is focused.
pub fn win_focused() -> i32 {
    unsafe { ffi::win_focused() }
}

/// Topmost window at a screen position, or `0`.
pub fn win_at(x: i32, y: i32) -> i32 {
    unsafe { ffi::win_at(x, y) }
}

/// Minimize, maximize or restore someone else's window.
pub fn win_set_state(window: i32, state: &str) -> bool {
    unsafe { ffi::win_set_state(window, state.as_ptr(), state.len() as i32) == 0 }
}

pub fn win_state(window: i32) -> String {
    read_string(|out, cap| unsafe { ffi::win_state(window, out, cap) }).unwrap_or_default()
}

/// Hit-test the screen and say *which part* of the window was hit — `(0, "")` when nothing
/// is there. The zone comes from the compositor because only it knows where the theme puts
/// the buttons and how wide the resize edges are.
pub fn win_hit_test(x: i32, y: i32) -> (i32, String) {
    let raw = read_string(|out, cap| unsafe { ffi::win_hit_test(x, y, out, cap) })
        .unwrap_or_default();
    // Both halves travel in one buffer, as "<id>\t<zone>", so they describe one instant.
    let mut parts = raw.split('\t');
    let window = parts.next().and_then(|id| id.parse().ok()).unwrap_or(0);
    (window, String::from(parts.next().unwrap_or_default()))
}

/// Reserve part of the screen so maximized windows stop short of the taskbar.
pub fn win_set_work_area(x: i32, y: i32, w: i32, h: i32) -> bool {
    unsafe { ffi::win_set_work_area(x, y, w, h) == 0 }
}

/// Ask for a different cursor shape, by `.pointer` entry name. Reset to `"default"` at the
/// end of every frame, so a shell must keep asking for as long as it wants it.
pub fn set_cursor(name: &str) -> bool {
    unsafe { ffi::set_cursor(name.as_ptr(), name.len() as i32) == 0 }
}

/// `(title_bar_height, border_width)` the compositor is drawing with, so a shell's drag
/// regions line up with the chrome the user actually sees.
pub fn chrome_metrics() -> (i32, i32) {
    unsafe { (ffi::chrome_title_height(), ffi::chrome_border_width()) }
}
