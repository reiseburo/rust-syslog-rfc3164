//! In-memory representation of a single Syslog message.

use std::string::String;

use serde::{Serializer, Serialize};

#[allow(non_camel_case_types)]
pub type time_t = i64;
#[allow(non_camel_case_types)]
pub type pid_t = i32;

use severity;
use facility;

#[derive(Clone,Debug,PartialEq,Eq)]
/// `ProcID`s are usually numeric PIDs; however, on some systems, they may be something else
pub enum ProcIdType {
    PID(pid_t),
    Name(String)
}


impl Serialize for ProcIdType {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match *self {
            ProcIdType::PID(ref p) => ser.serialize_i32(*p),
            ProcIdType::Name(ref n) => ser.serialize_str(n),
        }
    }
}

#[derive(Clone,Debug,Serialize)]
pub struct SyslogMessage {
    pub severity: severity::SyslogSeverity,
    pub facility: facility::SyslogFacility,
    pub version: i32,
    pub timestamp: Option<time_t>,
    pub hostname: Option<String>,
    pub proc_id: Option<ProcIdType>,
    pub tag: Option<String>,
    pub msg: String,
}


#[cfg(test)]
mod tests {
    use serde_json;
    use super::SyslogMessage;
    use severity::SyslogSeverity::*;
    use facility::SyslogFacility::*;

    #[test]
    fn test_serialization_serde() {
        let m = SyslogMessage {
            severity: SEV_INFO,
            facility: LOG_KERN,
            version: 1,
            timestamp: None,
            hostname: None,
            proc_id: None,
            tag: None,
            msg: String::from("")
        };

        let encoded = serde_json::to_string(&m).expect("Should encode to JSON");
//        println!("{:?}", encoded);
        // XXX: we don't have a guaranteed order, I don't think, so this might break with minor
        // version changes. *shrug*
        assert_eq!(encoded, "{\"severity\":\"info\",\"facility\":\"kern\",\"version\":1,\"timestamp\":null,\"hostname\":null,\"proc_id\":null,\"tag\":null,\"msg\":\"\"}");
    }
}
