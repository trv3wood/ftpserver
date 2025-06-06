#![allow(unused)]
pub const COMMAND_OK: &'static str = "200";
pub const SYNTAX_ERROR_UNRECOGNIZED_COMMAND: &'static str = "500";
pub const SYNTAX_ERROR_PARAMETERS: &'static str = "501";
pub const COMMAND_NOT_IMPLEMENTED_SUPERFLOUS: &'static str = "202";
pub const COMMAND_NOT_IMPLEMENTED: &'static str = "502";
pub const COMMANDS_BAD_SEQUENCE: &'static str = "503";
pub const COMMAND_NOT_IMPLEMENTED_FOR_PARAMETER: &'static str = "504";
pub const REPLY_RESTART_MARKER: &'static str = "110";
pub const REPLY_SYSTEM_STATUS: &'static str = "211";
pub const DIRECTORY_STATUS: &'static str = "212";
pub const FILE_STATUS: &'static str = "213";
pub const HELP_MESSAGE: &'static str = "214";
pub const NAME_SYSTEM_TYPE: &'static str = "215";

pub const SERVICE_READY_IN_MINUTES: &'static str = "120";
pub const SERVICE_READY_FOR_NEW_USER: &'static str = "220";
pub const SERVICE_CLOSING_CONTROL_CONNECTION: &'static str = "221";
pub const SERVICE_NOT_AVAILABLE: &'static str = "421";
pub const DATA_CONNECTION_OPEN_TRANSFER_STARTING: &'static str = "125";
pub const DATA_CONNECTION_OPEN_NO_TRANSFER: &'static str = "225";
pub const ERROR_OPENING_DATA_CONNECTION: &'static str = "425";
pub const CLOSING_DATA_CONNECTION: &'static str = "226";
pub const TRANSFER_ABORTED: &'static str = "426";
pub const ENTERING_PASSIVE_MODE: &'static str = "227";

pub const USER_LOGGED_IN: &'static str = "230";
pub const NOT_LOGGED_IN: &'static str = "530";
pub const USER_NAME_OK: &'static str = "331";
pub const NEED_ACCOUNT_FOR_LOGIN: &'static str = "332";
pub const NEED_ACCOUNT_FOR_STORING_FILES: &'static str = "532";
pub const FILE_STATUS_OK_OPENING_DATA_CONNECTION: &'static str = "150";
pub const FILE_ACTION_COMPLETED: &'static str = "250";
pub const PATHNAME_CREATED: &'static str = "257";
pub const FILE_ACTION_NEEDS_FURTHER_INFO: &'static str = "350";
pub const FILE_ACTION_NOT_TAKEN: &'static str = "450";
pub const ACTION_NOT_TAKEN: &'static str = "550";
pub const ACTION_ABORTED_LOCAL_ERROR: &'static str = "451";
pub const ACTION_ABORTED_PAGE_TYPE_UNKNOWN: &'static str = "551";
pub const ACTION_NOT_TAKEN_INSUFFICIENT_STORAGE_SPACE: &'static str = "452";
pub const FILE_ACTION_ABORTED: &'static str = "552";
pub const ACTION_NOT_TAKEN_FILENAME_NOT_ALLOWED: &'static str = "553";
