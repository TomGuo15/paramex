use crate::common::crate_file;

#[test]
fn transfer_only_root_compatibility_modules_are_gone() {
    for path in ["src/metrics/mod.rs", "src/session/mod.rs"] {
        assert!(
            !crate_file(path).exists(),
            "Transfer-only compatibility module should not remain at crate root: {path}"
        );
    }

    let lib = crate::common::read_crate_file("src/lib.rs");
    for declaration in ["pub mod metrics;", "pub mod session;"] {
        assert!(
            !lib.contains(declaration),
            "crate root should not expose shallow compatibility module `{declaration}`"
        );
    }
}

#[test]
fn crate_root_has_no_compatibility_reexports() {
    let lib = crate::common::read_crate_file("src/lib.rs");

    for declaration in ["pub mod numerics {", "pub mod numpy_compat {"] {
        assert!(
            !lib.contains(declaration),
            "crate root should not expose shared compatibility module `{declaration}`"
        );
    }
    assert!(
        !lib.lines().any(|line| line.starts_with("pub use ")),
        "crate root should expose product/shared modules, not compatibility re-exports"
    );
}
