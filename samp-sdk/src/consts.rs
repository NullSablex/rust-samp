//! Default AMX constants.
use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Supports: u32 {
        const VERSION = 512;
        const AMX_NATIVES = 0x10000;
        const PROCESS_TICK = 0x20000;
    }
}

/// Offsets
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServerData {
    Logprintf = 0,
    AmxExports = 16,
    CallPublicFs = 17,
    CallPublicGm = 18,
}

impl From<ServerData> for isize {
    fn from(data: ServerData) -> isize {
        data as isize
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct AmxFlags: u16 {
        const DEBUG = 0x02;
        const COMPACT = 0x04;
        const BYTEOPC = 0x08;
        const NOCHECKS = 0x10;
        const NTVREG = 0x1000;
        const JITC = 0x2000;
        const BROWSE = 0x4000;
        const RELOC = 0x8000;
    }
}

/// Index of an AMX function in memory.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmxExecIdx {
    Main,
    Continue,
    UserDef(i32),
}

impl From<AmxExecIdx> for i32 {
    fn from(value: AmxExecIdx) -> i32 {
        match value {
            AmxExecIdx::Main => -1,
            AmxExecIdx::Continue => -2,
            AmxExecIdx::UserDef(idx) => idx,
        }
    }
}

impl From<i32> for AmxExecIdx {
    fn from(idx: i32) -> AmxExecIdx {
        match idx {
            -1 => AmxExecIdx::Main,
            -2 => AmxExecIdx::Continue,
            idx => AmxExecIdx::UserDef(idx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amx_exec_idx_main_roundtrip() {
        assert_eq!(i32::from(AmxExecIdx::Main), -1);
        assert_eq!(AmxExecIdx::from(-1), AmxExecIdx::Main);
    }

    #[test]
    fn amx_exec_idx_continue_roundtrip() {
        assert_eq!(i32::from(AmxExecIdx::Continue), -2);
        assert_eq!(AmxExecIdx::from(-2), AmxExecIdx::Continue);
    }

    #[test]
    fn amx_exec_idx_userdef_roundtrip() {
        for idx in [0, 1, 10, 100, i32::MAX] {
            assert_eq!(AmxExecIdx::from(idx), AmxExecIdx::UserDef(idx));
            assert_eq!(i32::from(AmxExecIdx::UserDef(idx)), idx);
        }
    }

    #[test]
    fn server_data_offsets() {
        assert_eq!(isize::from(ServerData::Logprintf), 0);
        assert_eq!(isize::from(ServerData::AmxExports), 16);
        assert_eq!(isize::from(ServerData::CallPublicFs), 17);
        assert_eq!(isize::from(ServerData::CallPublicGm), 18);
    }

    #[test]
    fn supports_flags_combine() {
        let flags = Supports::VERSION | Supports::AMX_NATIVES;
        assert!(flags.contains(Supports::VERSION));
        assert!(flags.contains(Supports::AMX_NATIVES));
        assert!(!flags.contains(Supports::PROCESS_TICK));
    }

    #[test]
    fn supports_with_process_tick() {
        let flags = Supports::VERSION | Supports::AMX_NATIVES | Supports::PROCESS_TICK;
        assert!(flags.contains(Supports::PROCESS_TICK));
    }

    #[test]
    fn amx_flags_combine() {
        let flags = AmxFlags::DEBUG | AmxFlags::COMPACT;
        assert!(flags.contains(AmxFlags::DEBUG));
        assert!(flags.contains(AmxFlags::COMPACT));
        assert!(!flags.contains(AmxFlags::JITC));
    }

    #[test]
    fn amx_flags_empty() {
        let flags = AmxFlags::empty();
        assert!(!flags.contains(AmxFlags::DEBUG));
        assert!(flags.is_empty());
    }
}
