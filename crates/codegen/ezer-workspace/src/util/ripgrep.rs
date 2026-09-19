// Resolution (bundled binary, RG_BIN_PATH, Bazel runfiles, PATH) lives in the ezer-tools crate
// This module only preserves the `crate::util::ripgrep` path
pub use ezer_tools::util::rg_path;
