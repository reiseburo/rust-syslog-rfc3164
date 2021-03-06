use std::str::FromStr;
use std::str;
use std::num;
use std::string;

use log::*;

use time;

use severity;
use facility;
use message::{time_t, ProcIdType, SyslogMessage};

#[derive(Debug)]
pub enum ParseErr {
    RegexDoesNotMatchErr,
    BadSeverityInPri,
    BadFacilityInPri,
    UnexpectedEndOfInput,
    MonthConversionErr(String),
    TooFewDigits,
    TooManyDigits,
    InvalidUTCOffset,
    BaseUnicodeError(str::Utf8Error),
    UnicodeError(string::FromUtf8Error),
    ExpectedTokenErr(char),
    IntConversionErr(num::ParseIntError),
    MissingField(&'static str),
}

// We parse with this super-duper-dinky hand-coded recursive descent parser because we don't really
// have much other choice:
//
//  - Regexp is much slower (at least a factor of 4), and we still end up having to parse the
//    somewhat-irregular SD
//  - LALRPOP requires non-ambiguous tokenization
//  - Rust-PEG doesn't work on anything except nightly
//
// So here we are. The macros make it a bit better.
//
// General convention is that the parse state is represented by a string slice named "rest"; the
// macros will update that slice as they consume tokens.

macro_rules! maybe_expect_char {
    ($s:expr, $e: expr) => (match $s.chars().next() {
        Some($e) => Some(&$s[1..]),
        _ => None,
    })
}
// maybe_take_item!(parse_num(rest, 4, 4), maybe_rest)
macro_rules! maybe_take_item {
    ($e:expr, $r:expr) => {{
        match $e {
            Ok((v,r)) => {
                $r = r;
                Some(v)
            },
            Err(_) => {
                None
            }
        }
    }}
}

macro_rules! take_item {
    ($e:expr, $r:expr) => {{
        let (t, r) = $e?;
        $r = r;
        t
    }}
}

type ParseResult<T> = Result<T, ParseErr>;

macro_rules! take_char {
    ($e: expr, $c:expr) => {{
        $e = match $e.chars().next() {
            Some($c) => &$e[1..],
            Some(_) => {
                //debug!("Error with rest={:?}", $e);
                return Err(ParseErr::ExpectedTokenErr($c));
            },
            None => {
                //debug!("Error with rest={:?}", $e);
                return Err(ParseErr::UnexpectedEndOfInput);
            }
        }
    }}
}

fn take_while<F>(input: &str, f: F, max_chars: usize) -> (&str, Option<&str>)
where
    F: Fn(char) -> bool,
{
    for (idx, chr) in input.char_indices() {
        if !f(chr) {
            return (&input[..idx], Some(&input[idx..]));
        }
        if idx == max_chars {
            return (&input[..idx], Some(&input[idx..]));
        }
    }
    ("", None)
}

fn parse_pri_val(pri: i32) -> ParseResult<(severity::SyslogSeverity, facility::SyslogFacility)> {
    let sev = severity::SyslogSeverity::from_int(pri & 0x7).ok_or(ParseErr::BadSeverityInPri)?;
    let fac = facility::SyslogFacility::from_int(pri >> 3).ok_or(ParseErr::BadFacilityInPri)?;
    Ok((sev, fac))
}

fn parse_month(s: &str) -> ParseResult<(i32, &str)> {
    let (res, rest1) = take_while(s, |c| c >= 'A' && c <= 'z', 3);
    let rest = rest1.ok_or(ParseErr::UnexpectedEndOfInput)?;

    match res {
        "Jan" => Ok((1, rest)),
        "Feb" => Ok((2, rest)),
        "Mar" => Ok((3, rest)),
        "Apr" => Ok((4, rest)),
        "May" => Ok((5, rest)),
        "Jun" => Ok((6, rest)),
        "Jul" => Ok((7, rest)),
        "Aug" => Ok((8, rest)),
        "Sep" => Ok((9, rest)),
        "Oct" => Ok((10, rest)),
        "Nov" => Ok((11, rest)),
        "Dec" => Ok((12, rest)),
        _ => Err(ParseErr::MonthConversionErr(res.into())),
    }
}

fn parse_num(s: &str, min_digits: usize, max_digits: usize) -> ParseResult<(i32, &str)> {
    let (res, rest1) = take_while(s, |c| c >= '0' && c <= '9', max_digits);
    let rest = rest1.ok_or(ParseErr::UnexpectedEndOfInput)?;
    if res.len() < min_digits {
        Err(ParseErr::TooFewDigits)
    } else if res.len() > max_digits {
        Err(ParseErr::TooManyDigits)
    } else {
        Ok((
            i32::from_str(res).map_err(ParseErr::IntConversionErr)?,
            rest,
        ))
    }
}

fn parse_timestamp(m: &str) -> ParseResult<(Option<time_t>, &str)> {
    // Jan 8 12:14:16
    let mut rest = m;
    if rest.starts_with('-') {
        return Ok((None, &rest[1..]));
    }

    let mut tm = time::empty_tm();
    tm.tm_mon = take_item!(parse_month(rest), rest) - 1;
    take_char!(rest, ' ');
    rest = maybe_expect_char!(rest, ' ').unwrap_or(rest);
    tm.tm_mday = take_item!(parse_num(rest, 1, 2), rest);
    take_char!(rest, ' ');
    tm.tm_hour = take_item!(parse_num(rest, 2, 2), rest);
    take_char!(rest, ':');

    tm.tm_min = take_item!(parse_num(rest, 2, 2), rest);
    take_char!(rest, ':');
    tm.tm_sec = take_item!(parse_num(rest, 2, 2), rest);

    let mut maybe_rest = rest;
    maybe_rest = maybe_expect_char!(maybe_rest, ' ').unwrap_or(maybe_rest);
    match maybe_take_item!(parse_num(maybe_rest, 4, 4), maybe_rest) {
        Some(year) => {
            tm.tm_year = year - 1900;
            rest = maybe_rest;
        }
        None => {
            tm.tm_year = time::now().tm_year;
        }
    }

    Ok((Some(tm.to_utc().to_timespec().sec), rest))
}

fn parse_term(
    m: &str,
    min_length: usize,
    max_length: usize,
) -> ParseResult<(Option<String>, &str)> {
    if m.starts_with('-') {
        return Ok((None, &m[1..]));
    }
    let byte_ary = m.as_bytes();
    for (idx, chr) in byte_ary.iter().enumerate() {
        //debug!("idx={:?}, buf={:?}, chr={:?}", idx, buf, chr);
        debug!("doo {}", chr);
        if *chr < 33 || *chr > 126 {
            if idx < min_length {
                return Err(ParseErr::TooFewDigits);
            }
            let utf8_ary = str::from_utf8(&byte_ary[..idx]).map_err(ParseErr::BaseUnicodeError)?;
            return Ok((Some(String::from(utf8_ary)), &m[idx..]));
        }
        if idx >= max_length {
            let utf8_ary = str::from_utf8(&byte_ary[..idx]).map_err(ParseErr::BaseUnicodeError)?;
            return Ok((Some(String::from(utf8_ary)), &m[idx..]));
        }
    }
    debug!("no term found");
    Ok((None, &m[0..]))
}

fn parse_hostname(m: &str) -> ParseResult<(Option<String>, &str)> {
    let min_length = 1;
    let max_length = 255;
    if m.starts_with('-') {
        return Ok((None, &m[1..]));
    }
    let byte_ary = m.as_bytes();
    for (idx, chr) in byte_ary.iter().enumerate() {
        //        debug!("idx={:?}, buf={:?}, chr={:?}", idx, &m[0..idx], chr);
        if (*chr < 33 || *chr > 126) && (*chr != 91 || *chr == 93) {
            if idx < min_length {
                return Err(ParseErr::TooFewDigits);
            }
            let utf8_ary = str::from_utf8(&byte_ary[..idx]).map_err(ParseErr::BaseUnicodeError)?;
            return Ok((Some(String::from(utf8_ary)), &m[idx..]));
        }
        if idx >= max_length || *chr == 91 || *chr == 93 {
            let utf8_ary = str::from_utf8(&byte_ary[..idx]).map_err(ParseErr::BaseUnicodeError)?;
            return Ok((Some(String::from(utf8_ary)), &m[idx..]));
        }
    }
    Err(ParseErr::UnexpectedEndOfInput)
}

fn parse_message_s(m: &str) -> ParseResult<SyslogMessage> {
    let mut rest = m;
    take_char!(rest, '<');
    let prival = take_item!(parse_num(rest, 1, 3), rest);
    take_char!(rest, '>');
    let (sev, fac) = parse_pri_val(prival)?;
    // let version = take_item!(parse_num(rest, 1, 2), rest); // TODO: Nuke
    //debug!("got version {:?}, rest={:?}", version, rest);
    let timestamp = take_item!(parse_timestamp(rest), rest);
    debug!("timestampe: {:?}", timestamp);
    take_char!(rest, ' ');
    let hostname = take_item!(parse_hostname(rest), rest);
    rest = maybe_expect_char!(rest, '[').unwrap_or(rest);
    debug!("hostname: {:?}, rest={}", hostname, rest);
    rest = maybe_expect_char!(rest, ' ').unwrap_or(rest);

    let mut maybe_rest = rest;
    let proc_id: Option<ProcIdType> = match maybe_take_item!(parse_hostname(rest), maybe_rest) {
        Some(Some(proc_id_r)) => {
            debug!("pro: {}", proc_id_r);
            let res = Some(match i32::from_str(&proc_id_r) {
                Ok(n) => ProcIdType::PID(n),
                Err(_) => ProcIdType::Name(proc_id_r),
            });
            // Consume the trailing space before the content part of the message
            rest = maybe_expect_char!(maybe_rest, ' ').unwrap_or(maybe_rest);
            res
        }
        _ => None,
    };
    debug!("got hostname {:?}, rest={:?}", hostname, rest);
    let tag = take_item!(parse_term(rest, 1, 255), rest);
    debug!("got tag {:?} rest={:?}", tag, rest);
    rest = maybe_expect_char!(rest, ' ').unwrap_or(rest);

    let msg = String::from(rest);
    debug!("msg: {}", msg);

    Ok(SyslogMessage {
        severity: sev,
        facility: fac,
        version: 0,
        timestamp: timestamp,
        hostname: hostname,
        proc_id: proc_id,
        tag: tag,
        msg: msg,
    })
}

/// Parse a string into a `SyslogMessage` object
///
/// # Arguments
///
///  * `s`: Anything convertible to a string
///
/// # Returns
///
///  * `ParseErr` if the string is not parseable as an RFC5424 message
///
/// # Example
///
/// ```
/// use syslog_rfc3164::parse_message;
///
/// let message = parse_message("<78>Mar 15 14:16:22 host1 CROND 10391 - [meta sequenceId=\"29\"] some_message").unwrap();
///
/// assert!(message.hostname.unwrap() == "host1");
/// ```
pub fn parse_message<S: AsRef<str>>(s: S) -> ParseResult<SyslogMessage> {
    parse_message_s(s.as_ref())
}

#[cfg(test)]
mod tests {
    use super::{parse_hostname, parse_message, ProcIdType};
    use message;

    use facility::SyslogFacility;
    use severity::SyslogSeverity;

    use time;

    #[test]
    fn test_simple() {
        let msg = parse_message("<1>- - - - - -").expect("Should parse empty message");
        assert!(msg.facility == SyslogFacility::LOG_KERN);
        assert!(msg.severity == SyslogSeverity::SEV_ALERT);
        assert!(msg.timestamp.is_none());
        assert!(msg.hostname.is_none());
    }

    #[test]
    fn test_timestamp_without_year() {
        let msg: message::SyslogMessage =
            parse_message("<1>Jan 8 12:14:16 host tag -").expect("Should parse empty message");
        let mut tm = time::empty_tm();
        tm.tm_mon = 0;
        tm.tm_mday = 8;
        tm.tm_hour = 12;
        tm.tm_min = 14;
        tm.tm_sec = 16;
        tm.tm_year = time::now().tm_year;

        assert_eq!(msg.timestamp, Some(tm.to_utc().to_timespec().sec));
        assert_eq!(msg.hostname, Some("host".into()));
    }

    #[test]
    fn test_timestamp_with_year_in_message() {
        let msg = parse_message("<1>Jan 8 12:14:16 1995 host - - - -")
            .expect("Should parse empty message");
        assert_eq!(msg.timestamp, Some(789567256));
    }

    #[test]
    fn test_parsing_host_and_rest() {
        let data = "host1[123]";
        let res = parse_hostname(&data);
        let (hostname, procid) = res.unwrap();
        assert_eq!(hostname.unwrap(), "host1".to_owned());
        assert_eq!(procid, "[123]".to_owned());
    }

    #[test]
    fn test_complex() {
        let msg = parse_message("<78>Jan  8 12:14:16 2017 host1[123] CROND some_message")
            .expect("Should parse complex message");
        assert_eq!(msg.facility, SyslogFacility::LOG_CRON);
        assert_eq!(msg.severity, SyslogSeverity::SEV_INFO);
        assert_eq!(msg.hostname, Some(String::from("host1")));
        assert_eq!(msg.proc_id, Some(ProcIdType::PID(123)));
        assert_eq!(msg.msg, String::from("CROND some_message"));
        assert_eq!(msg.timestamp, Some(1483877656));
    }

    #[test]
    fn test_other_message() {
        let msg_text = r#"<190>Jan 8 12:14:16 batch6sj - - - [meta sequenceId="21881798" x-group="37051387"][origin x-service="tracking"] metascutellar conversationalist nephralgic exogenetic graphy streng outtaken acouasm amateurism prenotice Lyonese bedull antigrammatical diosphenol gastriloquial bayoneteer sweetener naggy roughhouser dighter addend sulphacid uneffectless ferroprussiate reveal Mazdaist plaudite Australasian distributival wiseman rumness Seidel topazine shahdom sinsion mesmerically pinguedinous ophthalmotonometer scuppler wound eciliate expectedly carriwitchet dictatorialism bindweb pyelitic idic atule kokoon poultryproof rusticial seedlip nitrosate splenadenoma holobenthic uneternal Phocaean epigenic doubtlessly indirection torticollar robomb adoptedly outspeak wappenschawing talalgia Goop domitic savola unstrafed carded unmagnified mythologically orchester obliteration imperialine undisobeyed galvanoplastical cycloplegia quinquennia foremean umbonal marcgraviaceous happenstance theoretical necropoles wayworn Igbira pseudoangelic raising unfrounced lamasary centaurial Japanolatry microlepidoptera"#;
        parse_message(msg_text).expect("should parse as text");
    }

    #[test]
    fn test_bad_pri() {
        let msg = parse_message("<4096>Jan 8 12:14:16 - - - - - -");
        assert!(msg.is_err());
    }

    #[test]
    fn test_good_match() {
        // we should be able to parse RFC3164 messages
        let msg = parse_message("<134>Feb 18 20:53:31 hostname.local nginx: I am a message");
        assert!(!msg.is_err());
    }

    #[test]
    fn test_good_matchy() {
        // we should be able to parse RFC3164 messages
        let msg = parse_message("<190>May 13 21:45:18 coconut hotdog: hi");
        assert!(!msg.is_err());
    }
}
