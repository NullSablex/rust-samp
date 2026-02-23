//! Work with AMX errors.
use std::error::Error;
use std::fmt::{self, Display};

/// A specialized [`Result`] type for operations on AMX.
///
/// [`Result`]: https://doc.rust-lang.org/std/result/enum.Result.html
pub type AmxResult<T> = Result<T, AmxError>;

/// Error type returned by AMX functions (origin amx_*).
#[derive(Debug)]
pub enum AmxError {
    Exit = 1,
    Assert = 2,
    StackError = 3,
    Bounds = 4,
    MemoryAccess = 5,
    InvalidInstruction = 6,
    StackLow = 7,
    HeapLow = 8,
    Callback = 9,
    Native = 10,
    Divide = 11,
    Sleep = 12,
    InvalidState = 13,
    Memory = 16,
    Format = 17,
    Version = 18,
    NotFound = 19,
    Index = 20,
    Debug = 21,
    Init = 22,
    UserData = 23,
    InitJit = 24,
    Params = 25,
    Domain = 26,
    General = 27,
    Overlay = 28,
    Unknown,
}

impl Display for AmxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::AmxError::*;

        match self {
            Exit => write!(f, "Forced exit"),
            Assert => write!(f, "Assertion failed"),
            StackError => write!(f, "Stack / heap collision"),
            Bounds => write!(f, "Index out of bounds"),
            MemoryAccess => write!(f, "Invalid memory access"),
            InvalidInstruction => write!(f, "Invalid instruction"),
            StackLow => write!(f, "Stack underflow"),
            HeapLow => write!(f, "Heap underflow"),
            Callback => write!(f, "No callback or invalid callback"),
            Native => write!(f, "Native function failed"),
            Divide => write!(f, "Divide by zero"),
            Sleep => write!(f, "Go into sleepmode"),
            InvalidState => write!(f, "No implementation for this state, no fall-back"),
            Memory => write!(f, "Out of memory"),
            Format => write!(f, "Invalid file format"),
            Version => write!(f, "File is for a newer version of AMX"),
            NotFound => write!(f, "Function not found"),
            Index => write!(f, "Invalid index parameter (bad entry point)"),
            Debug => write!(f, "Debbuger cannot run"),
            Init => write!(f, "AMX not initialize"),
            UserData => write!(f, "Unable to set user data field"),
            InitJit => write!(f, "Cannot initialize the JIT"),
            Params => write!(f, "Parameter error"),
            Domain => write!(f, "Domain error, expression result does not fit in range"),
            General => write!(f, "General error (unknown or unspecific error)"),
            Overlay => write!(f, "Overlays are unsupported (JIT) or uninitialized"),
            Unknown => write!(f, "Unknown error"),
        }
    }
}

impl Error for AmxError {}

impl From<i32> for AmxError {
    fn from(error_code: i32) -> Self {
        match error_code {
            1 => AmxError::Exit,
            2 => AmxError::Assert,
            3 => AmxError::StackError,
            4 => AmxError::Bounds,
            5 => AmxError::MemoryAccess,
            6 => AmxError::InvalidInstruction,
            7 => AmxError::StackLow,
            8 => AmxError::HeapLow,
            9 => AmxError::Callback,
            10 => AmxError::Native,
            11 => AmxError::Divide,
            12 => AmxError::Sleep,
            13 => AmxError::InvalidState,
            16 => AmxError::Memory,
            17 => AmxError::Format,
            18 => AmxError::Version,
            19 => AmxError::NotFound,
            20 => AmxError::Index,
            21 => AmxError::Debug,
            22 => AmxError::Init,
            23 => AmxError::UserData,
            24 => AmxError::InitJit,
            25 => AmxError::Params,
            26 => AmxError::Domain,
            27 => AmxError::General,
            28 => AmxError::Overlay,
            _ => AmxError::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_code_maps_all_known_errors() {
        let cases: &[(i32, &str)] = &[
            (1, "Exit"),
            (2, "Assert"),
            (3, "StackError"),
            (4, "Bounds"),
            (5, "MemoryAccess"),
            (6, "InvalidInstruction"),
            (7, "StackLow"),
            (8, "HeapLow"),
            (9, "Callback"),
            (10, "Native"),
            (11, "Divide"),
            (12, "Sleep"),
            (13, "InvalidState"),
            (16, "Memory"),
            (17, "Format"),
            (18, "Version"),
            (19, "NotFound"),
            (20, "Index"),
            (21, "Debug"),
            (22, "Init"),
            (23, "UserData"),
            (24, "InitJit"),
            (25, "Params"),
            (26, "Domain"),
            (27, "General"),
            (28, "Overlay"),
        ];

        for &(code, expected_name) in cases {
            let err = AmxError::from(code);
            assert_eq!(
                format!("{err:?}"),
                expected_name,
                "código {code} deveria mapear para {expected_name}"
            );
        }
    }

    #[test]
    fn unknown_codes_map_to_unknown() {
        for code in [0, 14, 15, 29, 100, -1, i32::MAX] {
            assert!(
                matches!(AmxError::from(code), AmxError::Unknown),
                "código {code} deveria ser Unknown"
            );
        }
    }

    #[test]
    fn display_messages_are_not_empty() {
        let errors = [
            AmxError::Exit,
            AmxError::Bounds,
            AmxError::Divide,
            AmxError::NotFound,
            AmxError::Unknown,
        ];

        for err in errors {
            let msg = format!("{err}");
            assert!(!msg.is_empty(), "{err:?} tem mensagem vazia");
        }
    }

    #[test]
    fn implements_std_error() {
        let err = AmxError::General;
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn memory_access_display() {
        let err = AmxError::MemoryAccess;
        assert_eq!(format!("{err}"), "Invalid memory access");
    }

    #[test]
    fn memory_error_display() {
        let err = AmxError::Memory;
        assert_eq!(format!("{err}"), "Out of memory");
    }

    #[test]
    fn amx_result_ok() {
        let result: AmxResult<i32> = Ok(42);
        assert!(result.is_ok());
    }

    #[test]
    fn amx_result_err() {
        let result: AmxResult<i32> = Err(AmxError::General);
        assert!(result.is_err());
        let err = AmxError::General;
        assert_eq!(format!("{err}"), "General error (unknown or unspecific error)");
    }
}
