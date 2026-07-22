//! The contract between the RetrOS core and `kind = "mixin"` packages.
//!
//! A mixin is native code loaded into the core process at boot. Unlike WASM/Python/Lua
//! apps it is **not sandboxed** — see plans.md §4.5.
//!
//! # Why a vtable and not `Box<dyn CoreMixin>`
//!
//! Rust has no stable ABI: a trait object built by one rustc version cannot be safely
//! consumed by another. plans.md §12 flags this as a real risk. So the *boundary* here is
//! a `#[repr(C)]` struct of `extern "C"` function pointers carrying an explicit
//! [`ABI_VERSION`], while mixin authors still write ordinary safe Rust by implementing
//! [`CoreMixin`] and invoking [`declare_mixin!`], which generates the shims.
//!
//! Core refuses to load any mixin whose `abi_version` does not match exactly.

#![deny(unsafe_op_in_unsafe_fn)]

use core::ffi::{c_char, c_void};

/// Bumped whenever anything in [`MixinVTable`] or [`DisplayConfigC`] changes shape.
pub const ABI_VERSION: u32 = 1;

/// The symbol every mixin `cdylib` must export.
pub const ENTRY_SYMBOL: &[u8] = b"retros_mixin_entry";

/// How a fixed internal resolution is scaled up to the host window.
pub mod scale_mode {
    pub const NEAREST: u32 = 0;
    pub const SMOOTH: u32 = 1;
}

/// Post-process effects the core knows how to apply.
pub mod overlay {
    pub const NONE: u32 = 0;
    pub const SCANLINES: u32 = 1;
    pub const PHOSPHOR_GLOW: u32 = 2;
}

/// The numeric subset of the core's display configuration that a mixin may rewrite.
///
/// String-valued fields (`palette`, `font`) are deliberately absent: passing owned strings
/// across the boundary would mean sharing an allocator, which is exactly the ABI coupling
/// this design avoids. A mixin that needs a different palette or font should ship
/// alongside a display profile package, which can express both safely.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayConfigC {
    /// `0` = follow the host window's size, `1` = use `width`/`height` as a fixed
    /// internal resolution that is scaled and letterboxed on present.
    pub fixed_resolution: u32,
    pub width: u32,
    pub height: u32,
    /// One of [`scale_mode`].
    pub scale_mode: u32,
    /// One of [`overlay`].
    pub overlay: u32,
    pub overlay_intensity: f32,
    /// `1` = only ever scale by whole numbers, so pixels stay square and uniform.
    pub integer_scale: u32,
}

/// A mutable view of the core's framebuffer, handed to `post_process_frame`.
///
/// `pixels` points at `width * height` `u32`s in `0x00RRGGBB` order, row-major.
#[repr(C)]
#[derive(Debug)]
pub struct FrameBufferC {
    pub pixels: *mut u32,
    pub width: u32,
    pub height: u32,
}

/// The exported entry point's return type. Owned by the mixin, freed via `drop_instance`.
#[repr(C)]
#[derive(Debug)]
pub struct MixinVTable {
    /// Must equal [`ABI_VERSION`] or core will refuse to load the mixin.
    pub abi_version: u32,
    /// NUL-terminated, `'static` for the lifetime of the loaded library.
    pub name: *const c_char,
    /// Opaque pointer to the author's mixin value; passed back to every hook.
    pub instance: *mut c_void,

    pub on_boot: Option<extern "C" fn(*mut c_void)>,
    pub configure_display: Option<extern "C" fn(*mut c_void, *mut DisplayConfigC)>,
    pub post_process_frame: Option<extern "C" fn(*mut c_void, *mut FrameBufferC)>,
    pub drop_instance: Option<extern "C" fn(*mut c_void)>,
}

/// The signature of `retros_mixin_entry`.
pub type MixinEntry = unsafe extern "C" fn() -> *mut MixinVTable;

/// A safe, mutable view over the core framebuffer.
pub struct FrameBuffer<'a> {
    width: u32,
    height: u32,
    pixels: &'a mut [u32],
}

impl<'a> FrameBuffer<'a> {
    /// # Safety
    /// `raw` must describe a live, uniquely-borrowed buffer of `width * height` `u32`s.
    pub unsafe fn from_raw(raw: &'a mut FrameBufferC) -> Self {
        let len = (raw.width as usize).saturating_mul(raw.height as usize);
        let pixels = unsafe { core::slice::from_raw_parts_mut(raw.pixels, len) };
        Self { width: raw.width, height: raw.height, pixels }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u32] {
        self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [u32] {
        self.pixels
    }

    pub fn get(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.pixels[(y * self.width + x) as usize])
    }

    pub fn set(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize] = color;
        }
    }
}

/// The ergonomic trait a mixin author implements. Every hook has a no-op default, so a
/// mixin only writes the ones it declares in `[mixin] hooks = [...]`.
pub trait CoreMixin {
    /// Runs once, before the host window is created.
    fn on_boot(&mut self) {}

    /// Rewrite the display configuration the core is about to use.
    fn configure_display(&mut self, base: DisplayConfigC) -> DisplayConfigC {
        base
    }

    /// Mutate the fully composited frame just before it is presented.
    fn post_process_frame(&mut self, _buf: &mut FrameBuffer<'_>) {}
}

#[doc(hidden)]
pub mod __private {
    pub use core::ffi::c_void;
}

/// Generates the `extern "C"` entry point and hook shims for a [`CoreMixin`] impl.
///
/// ```ignore
/// struct Curvature;
/// impl retros_mixin_api::CoreMixin for Curvature {
///     fn post_process_frame(&mut self, buf: &mut retros_mixin_api::FrameBuffer<'_>) { /* ... */ }
/// }
/// retros_mixin_api::declare_mixin!("crt-curvature-mixin", Curvature, Curvature);
/// ```
#[macro_export]
macro_rules! declare_mixin {
    ($name:expr, $ty:ty, $ctor:expr) => {
        const _: () = {
            type __Mixin = $ty;

            extern "C" fn __on_boot(instance: *mut $crate::__private::c_void) {
                let m = unsafe { &mut *(instance as *mut __Mixin) };
                $crate::CoreMixin::on_boot(m);
            }

            extern "C" fn __configure_display(
                instance: *mut $crate::__private::c_void,
                cfg: *mut $crate::DisplayConfigC,
            ) {
                let m = unsafe { &mut *(instance as *mut __Mixin) };
                let cfg = unsafe { &mut *cfg };
                *cfg = $crate::CoreMixin::configure_display(m, *cfg);
            }

            extern "C" fn __post_process_frame(
                instance: *mut $crate::__private::c_void,
                raw: *mut $crate::FrameBufferC,
            ) {
                let m = unsafe { &mut *(instance as *mut __Mixin) };
                let mut view = unsafe { $crate::FrameBuffer::from_raw(&mut *raw) };
                $crate::CoreMixin::post_process_frame(m, &mut view);
            }

            extern "C" fn __drop_instance(instance: *mut $crate::__private::c_void) {
                drop(unsafe { ::std::boxed::Box::from_raw(instance as *mut __Mixin) });
            }

            #[no_mangle]
            pub extern "C" fn retros_mixin_entry() -> *mut $crate::MixinVTable {
                let instance = ::std::boxed::Box::into_raw(::std::boxed::Box::new($ctor));
                ::std::boxed::Box::into_raw(::std::boxed::Box::new($crate::MixinVTable {
                    abi_version: $crate::ABI_VERSION,
                    name: ::std::concat!($name, "\0").as_ptr() as *const _,
                    instance: instance as *mut $crate::__private::c_void,
                    on_boot: Some(__on_boot),
                    configure_display: Some(__configure_display),
                    post_process_frame: Some(__post_process_frame),
                    drop_instance: Some(__drop_instance),
                }))
            }
        };
    };
}
