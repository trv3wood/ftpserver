#[derive(Debug, Clone, Copy)]
#[repr(u16)]
#[allow(unused)]
pub enum FtpReplyCode {
    // Reply codes from RFC 959 (plain FTP)
    // https://tools.ietf.org/html/rfc959
    CommandOk = 200,
    SyntaxErrorUnrecognizedCommand = 500,
    SyntaxErrorParameters = 501,
    CommandNotImplementedSuperflous = 202,
    CommandNotImplemented = 502,
    CommandsBadSequence = 503,
    CommandNotImplementedForParameter = 504,

    ReplyRestartMarker = 110,
    ReplySystemStatus = 211,
    DirectoryStatus = 212,
    FileStatus = 213,
    HelpMessage = 214,
    NameSystemType = 215,

    ServiceReadyInMinutes = 120,
    ServiceReadyForNewUser = 220,
    ServiceClosingControlConnection = 221,
    ServiceNotAvailable = 421,
    DataConnectionOpenTransferStarting = 125,
    DataConnectionOpenNoTransfer = 225,
    ErrorOpeningDataConnection = 425,
    ClosingDataConnection = 226,
    TransferAborted = 426,
    EnteringPassiveMode = 227,

    UserLoggedIn = 230,
    NotLoggedIn = 530,
    UserNameOk = 331,
    NeedAccountForLogin = 332,
    NeedAccountForStoringFiles = 532,
    FileStatusOkOpeningDataConnection = 150,
    FileActionCompleted = 250,
    PathnameCreated = 257,
    FileActionNeedsFurtherInfo = 350,
    FileActionNotTaken = 450,
    ActionNotTaken = 550,
    ActionAbortedLocalError = 451,
    ActionAbortedPageTypeUnknown = 551,
    ActionNotTakenInsufficientStorageSpace = 452,
    FileActionAborted = 552,
    ActionNotTakenFilenameNotAllowed = 553,
}

pub struct FtpMessage {
    code: FtpReplyCode,
    msg: String,
}

impl FtpMessage {
    pub fn new(code: FtpReplyCode, msg: &str) -> Self {
        Self {
            code,
            msg: msg.to_string(),
        }
    }
    pub fn string(&self) -> String {
        format!("{} {}\r\n", self.code as u16, self.msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ftpmsg() {
        assert_eq!(
            FtpMessage::new(FtpReplyCode::CommandOk, "Service ready for new user")
                .string()
                .as_str(),
            "200 Service ready for new user\r\n"
        );
    }
}
