//! Internal log macros with the SDK prefix.
//!
//! Avoids hardcoded `"[rust-samp]"` scattered across the crate — changing the
//! prefix in the future is a single edit in [`SDK_LOG_PREFIX`]. For now only
//! `sdk_warn!` is used; add `sdk_error!`/`sdk_info!` if a real need arises.

/// Prefix applied to all SDK log messages.
pub(crate) const SDK_LOG_PREFIX: &str = "[rust-samp]";

/// Equivalent to `log::warn!` but prepends [`SDK_LOG_PREFIX`].
macro_rules! sdk_warn {
    ($($arg:tt)*) => {
        log::warn!("{} {}", $crate::macros::SDK_LOG_PREFIX, format_args!($($arg)*))
    };
}

pub(crate) use sdk_warn;
