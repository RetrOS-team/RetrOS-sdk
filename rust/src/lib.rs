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

        // clipping and layering
        pub fn clip_push(x: i32, y: i32, w: i32, h: i32) -> i32;
        pub fn clip_pop() -> i32;
        pub fn clip_reset() -> i32;
        pub fn clip_x() -> i32;
        pub fn clip_y() -> i32;
        pub fn clip_w() -> i32;
        pub fn clip_h() -> i32;
        pub fn layer(n: i32) -> i32;
        pub fn current_layer() -> i32;

        // power (needs permissions.power)
        pub fn shutdown() -> i32;
        pub fn reboot() -> i32;

        // host handoff (needs permissions.host_open)
        pub fn host_open(ptr: *const u8, len: i32) -> i32;
        pub fn host_path(ptr: *const u8, len: i32, out: *mut u8, cap: i32) -> i32;

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
        pub fn rpkg(argv: *const u8, argv_len: i32, out: *mut u8, cap: i32) -> i32;
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

        // clipboard
        pub fn clipboard_get(out: *mut u8, cap: i32) -> i32;
        pub fn clipboard_set(ptr: *const u8, len: i32);

        // processes (needs permissions.process_manager)
        pub fn list_apps(out: *mut u8, cap: i32) -> i32;
        pub fn kill_app(target: i32) -> i32;
        pub fn kill_package(id: *const u8, id_len: i32) -> i32;

        // services (needs permissions.service_manager)
        pub fn service_list(out: *mut u8, cap: i32) -> i32;
        pub fn service_start(id: *const u8, id_len: i32) -> i32;
        pub fn service_stop(id: *const u8, id_len: i32) -> i32;
        pub fn service_enable(id: *const u8, id_len: i32, enabled: i32) -> i32;

        // network (needs permissions.network)
        pub fn http_request(
            method: *const u8,
            method_len: i32,
            url: *const u8,
            url_len: i32,
            body: *const u8,
            body_len: i32,
            out: *mut u8,
            cap: i32,
        ) -> i32;

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

// ------------------------------------------------- clipping and layering --
//
// A clip confines every primitive, `clear` included, so it fills only the clipped region
// — that is what makes it usable for a scrolling pane and not merely a guard. Nested
// clips intersect, so a pane can never draw outside its parent. Both the clip stack and
// the layer selection are reset when your tick returns.

/// Confine drawing to a rectangle in window coordinates, intersected with the clip
/// already in force. `false` when the stack is too deep to take another.
///
/// Prefer [`clip`], which pops for you.
pub fn clip_push(x: i32, y: i32, w: i32, h: i32) -> bool {
    unsafe { ffi::clip_push(x, y, w, h) == 0 }
}

pub fn clip_pop() -> bool {
    unsafe { ffi::clip_pop() == 0 }
}

pub fn clip_reset() -> bool {
    unsafe { ffi::clip_reset() == 0 }
}

/// `(x, y, w, h)` of the clip in force, already intersected with every enclosing one.
///
/// Four calls rather than one packed buffer, the way [`work_area`] is: nothing but this
/// app's own calls moves the clip, so the four cannot disagree with each other.
pub fn clip_rect() -> (i32, i32, i32, i32) {
    unsafe { (ffi::clip_x(), ffi::clip_y(), ffi::clip_w(), ffi::clip_h()) }
}

/// A clip that pops itself at the end of the scope, so an early `return` in the middle of
/// a pane cannot leave the rest of the frame drawing into a sliver.
///
/// ```ignore
/// let _pane = retros::clip(0, 16, w, h - 16);
/// draw_rows();
/// ```
pub struct Clip {
    /// A refused push must not pop on drop: it would discard the *enclosing* clip, which
    /// this app pushed for a reason.
    pushed: bool,
}

impl Drop for Clip {
    fn drop(&mut self) {
        if self.pushed {
            clip_pop();
        }
    }
}

/// Push a clip and get a guard that pops it. Binding it to `_` drops it immediately and
/// clips nothing, which is why the result must be named.
#[must_use = "the clip is popped as soon as the guard is dropped"]
pub fn clip(x: i32, y: i32, w: i32, h: i32) -> Clip {
    Clip { pushed: clip_push(x, y, w, h) }
}

/// Send subsequent drawing to layer `n`.
///
/// 0 draws immediately and costs nothing extra. Anything higher is recorded and replayed
/// after your tick in ascending order, which is how a drop-down or a modal lands above
/// widgets declared after it. `false` when `n` is above the engine's ceiling.
pub fn layer(n: i32) -> bool {
    unsafe { ffi::layer(n) == 0 }
}

pub fn current_layer() -> i32 {
    unsafe { ffi::current_layer() }
}

// -------------------------------------------------------------------- power --
//
// Both need `permissions.power` and return `false` without it. Both take effect once the
// current tick returns, so the caller keeps running to the end of its frame.

/// Stop RetrOS: every app is given its `shutdown` before the process exits.
pub fn shutdown() -> bool {
    unsafe { ffi::shutdown() == 0 }
}

/// Restart in place — config, theme, registry and fonts are rebuilt from disk without
/// leaving the process.
pub fn reboot() -> bool {
    unsafe { ffi::reboot() == 0 }
}

// ------------------------------------------------------------- host handoff --
//
// Both need `permissions.host_open`. The path is resolved through this app's own sandbox
// first, so it can only ever hand over something it could already read.

/// Ask the host operating system to open one of this app's paths with whatever it
/// considers the right program.
pub fn host_open(path: &str) -> bool {
    unsafe { ffi::host_open(path.as_ptr(), path.len() as i32) == 0 }
}

/// Where a sandbox path really lives on the host, so an app can show the user the path
/// they would type into a terminal.
pub fn host_path(path: &str) -> Option<String> {
    read_string(|out, cap| unsafe { ffi::host_path(path.as_ptr(), path.len() as i32, out, cap) })
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

/// Run `rpkg` and return its output. Requires `permissions.package_manager`, which
/// only the `console` package declares by default.
pub fn rpkg(args: &[&str]) -> Option<String> {
    let joined = args.join("\n");
    read_string(|out, cap| unsafe {
        ffi::rpkg(joined.as_ptr(), joined.len() as i32, out, cap)
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

// ---------------------------------------------------------------- clipboard --
//
// One plain-text buffer shared by every package, and no permission gates it: a clipboard
// that does not cross app boundaries is not a clipboard. Nothing secret belongs in it.

pub fn clipboard_get() -> String {
    read_string(|out, cap| unsafe { ffi::clipboard_get(out, cap) }).unwrap_or_default()
}

pub fn clipboard_set(text: &str) {
    unsafe { ffi::clipboard_set(text.as_ptr(), text.len() as i32) }
}

// ---------------------------------------------------------------- processes --
//
// Every call here needs `permissions.process_manager = true` in package.toml.

/// A running app and what it is costing.
pub struct AppStatus {
    pub id: i32,
    pub package: String,
    pub name: String,
    pub title: String,
    pub language: String,
    pub uptime_frames: u64,
    pub ticks: u64,
    /// Rolling average and worst case of one tick, in milliseconds. Everything runs on one
    /// thread, so this *is* the app's share of the frame.
    pub tick_ms: f64,
    pub tick_ms_peak: f64,
    /// A floor on resident size, not a total: interpreter heaps cannot be measured from
    /// the host, so this counts only what the kernel holds on the app's behalf.
    pub bytes: u64,
    pub window: i32,
    pub is_service: bool,
}

pub fn list_apps() -> Vec<AppStatus> {
    let raw = read_string(|out, cap| unsafe { ffi::list_apps(out, cap) }).unwrap_or_default();
    raw.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(AppStatus {
                id: parts.next()?.parse().ok()?,
                package: String::from(parts.next()?),
                name: String::from(parts.next()?),
                title: String::from(parts.next()?),
                language: String::from(parts.next()?),
                uptime_frames: parts.next()?.parse().ok()?,
                ticks: parts.next()?.parse().ok()?,
                tick_ms: parts.next()?.parse().ok()?,
                tick_ms_peak: parts.next()?.parse().ok()?,
                bytes: parts.next()?.parse().ok()?,
                window: parts.next()?.parse().ok()?,
                is_service: parts.next()? == "1",
            })
        })
        .collect()
}

/// Stop one running app by id. Queued like every other close, so the target is never torn
/// down while its own code is on the stack.
pub fn kill_app(target: i32) -> bool {
    unsafe { ffi::kill_app(target) == 0 }
}

/// Stop every app of one package. Returns how many were stopped, or `None` on refusal.
pub fn kill_package(id: &str) -> Option<u32> {
    let count = unsafe { ffi::kill_package(id.as_ptr(), id.len() as i32) };
    (count >= 0).then_some(count as u32)
}

// ----------------------------------------------------------------- services --
//
// Every call here needs `permissions.service_manager = true` in package.toml.

/// A service and what it is doing.
pub struct ServiceInfo {
    pub package: String,
    pub description: String,
    pub state: String,
    pub enabled: bool,
    pub restart: String,
    pub restarts: u32,
    pub last_error: String,
}

pub fn service_list() -> Vec<ServiceInfo> {
    let raw = read_string(|out, cap| unsafe { ffi::service_list(out, cap) }).unwrap_or_default();
    raw.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(ServiceInfo {
                package: String::from(parts.next()?),
                description: String::from(parts.next()?),
                state: String::from(parts.next()?),
                enabled: parts.next()? == "1",
                restart: String::from(parts.next()?),
                restarts: parts.next()?.parse().ok()?,
                // Empty when the service has never failed, and an error message may itself
                // be empty, so this is the one field allowed to be missing entirely.
                last_error: String::from(parts.next().unwrap_or_default()),
            })
        })
        .collect()
}

pub fn service_start(id: &str) -> bool {
    unsafe { ffi::service_start(id.as_ptr(), id.len() as i32) == 0 }
}

pub fn service_stop(id: &str) -> bool {
    unsafe { ffi::service_stop(id.as_ptr(), id.len() as i32) == 0 }
}

/// Turn a service on or off for future boots. Persisted immediately.
pub fn service_enable(id: &str, enabled: bool) -> bool {
    unsafe { ffi::service_enable(id.as_ptr(), id.len() as i32, enabled as i32) == 0 }
}

// ------------------------------------------------------------------ network --
//
// Needs `permissions.network = true` in package.toml.

/// Make an HTTP request; `None` when the permission is missing or the request failed.
///
/// Synchronous: the frame loop stops while the request is in flight.
pub fn http_request(method: &str, url: &str, body: &str) -> Option<(u16, String)> {
    let raw = read_string(|out, cap| unsafe {
        ffi::http_request(
            method.as_ptr(),
            method.len() as i32,
            url.as_ptr(),
            url.len() as i32,
            body.as_ptr(),
            body.len() as i32,
            out,
            cap,
        )
    })?;
    // Both halves travel in one buffer, as "<status>\t<body>", the same way `win_hit_test`
    // returns its pair: a wasm function returns one scalar, and re-issuing the request to
    // collect the other half would be a second request.
    let (status, body) = raw.split_once('\t')?;
    Some((status.parse().ok()?, String::from(body)))
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

// ------------------------------------------------------------------- events --
//
// Events reach a wasm app the long way round: the host has no allocator inside the guest,
// so it writes the event name and value into memory the *app* owns and calls `on_event`
// with offsets into it. The app advertises that memory by exporting
// `retros_event_scratch` and `retros_event_scratch_len` — and an app that exports neither
// is silently never sent an event at all, which looks exactly like an engine bug.

/// Declare the buffer the host delivers events into. Every app with an `on_event` needs
/// this once, at module level.
///
/// ```ignore
/// retros::event_scratch!();
///
/// #[retros::main]
/// fn on_event(name: i32, name_len: i32, arg: i32, arg_len: i32) {
///     let name = retros::event_str(name, name_len);
///     let arg = retros::event_str(arg, arg_len);
/// }
/// ```
///
/// The default holds 256 bytes for the name and value together; pass a size to take more.
/// An event that does not fit is dropped rather than truncated.
#[macro_export]
macro_rules! event_scratch {
    () => {
        $crate::event_scratch!(256);
    };
    ($capacity:expr) => {
        const __RETROS_EVENT_SCRATCH_LEN: usize = $capacity;
        static mut __RETROS_EVENT_SCRATCH: [u8; __RETROS_EVENT_SCRATCH_LEN] =
            [0; __RETROS_EVENT_SCRATCH_LEN];

        // `unsafe(no_mangle)` for the same reason `#[retros::main]` uses it: these tokens
        // carry the SDK's edition, which is 2024, where the bare form is rejected.
        #[unsafe(no_mangle)]
        pub extern "C" fn retros_event_scratch() -> i32 {
            // A raw pointer, never a reference: `&static mut` is undefined behaviour the
            // moment the host writes through it.
            (&raw const __RETROS_EVENT_SCRATCH) as i32
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn retros_event_scratch_len() -> i32 {
            __RETROS_EVENT_SCRATCH_LEN as i32
        }
    };
}

/// Borrow one of the strings the host just wrote into the scratch buffer.
///
/// Valid for exactly as long as the `on_event` call: the bytes were written immediately
/// before it and the next event overwrites them. Copy anything you intend to keep.
pub fn event_str<'a>(ptr: i32, len: i32) -> &'a str {
    if ptr <= 0 || len <= 0 {
        return "";
    }
    // Safe under the contract above, and non-UTF-8 is impossible: the host only ever
    // writes `&str` bytes here.
    unsafe {
        let bytes = core::slice::from_raw_parts(ptr as *const u8, len as usize);
        core::str::from_utf8(bytes).unwrap_or("")
    }
}
