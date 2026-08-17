//! Build script: embed the Windows `.exe` icon so `ParamEx.exe` shows the ParamEx
//! mark in Explorer/taskbar instead of the generic exe icon.
//!
//! Build-time only — no effect on runtime behavior or non-Windows hosts. A missing
//! resource compiler degrades to a cargo warning (the exe just ships without the
//! embedded icon) rather than breaking the build.

fn main() {
    // Re-run if the icon changes (path is relative to this crate's manifest dir).
    println!("cargo:rerun-if-changed=../../packaging/windows/paramex.ico");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/windows/paramex.ico");
        if let Err(err) = res.compile() {
            println!("cargo:warning=failed to embed exe icon (paramex.ico): {err}");
        }
    }
}
