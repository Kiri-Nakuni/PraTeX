use chrono::{DateTime, Datelike, FixedOffset, Offset, Timelike, Utc};
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Display, Formatter};

const SOURCE_DATE_EPOCH: &str = "SOURCE_DATE_EPOCH";

/// 一回のTeX runが共有する暦時刻。
///
/// TeXの整数parameterは利用者が代入できるため、PDF metadataやDVI commentが
/// 途中の代入に引きずられないよう、取得時点の値とUTC offsetを別に保持する。
/// fmtへdumpせず、run開始時にだけEqtbへ設定する。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RunDateTime {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    utc_offset_minutes: i32,
}

impl RunDateTime {
    /// 環境を一度だけ読み、run全体の時計を固定する。
    pub(crate) fn capture() -> Result<Self, RuntimeClockError> {
        match std::env::var_os(SOURCE_DATE_EPOCH) {
            Some(value) => Self::from_source_date_epoch(&value),
            None => Self::capture_local(),
        }
    }

    /// 再現buildのepochは仕様どおりUTCとして解釈する。
    fn from_source_date_epoch(value: &OsStr) -> Result<Self, RuntimeClockError> {
        let text = value
            .to_str()
            .ok_or_else(|| RuntimeClockError::InvalidSourceDateEpoch(value.to_os_string()))?;
        if text.is_empty()
            || text
                .bytes()
                .enumerate()
                .any(|(position, byte)| !byte.is_ascii_digit() && !(position == 0 && byte == b'-'))
        {
            return Err(RuntimeClockError::InvalidSourceDateEpoch(
                value.to_os_string(),
            ));
        }
        let seconds = text
            .parse::<i64>()
            .map_err(|_| RuntimeClockError::InvalidSourceDateEpoch(value.to_os_string()))?;
        let datetime = DateTime::<Utc>::from_timestamp(seconds, 0)
            .ok_or(RuntimeClockError::SourceDateEpochOutOfRange(seconds))?;
        Self::from_datetime(datetime.fixed_offset())
    }

    #[cfg(any(unix, windows))]
    fn capture_local() -> Result<Self, RuntimeClockError> {
        Self::from_datetime(chrono::Local::now().fixed_offset())
    }

    #[cfg(not(any(unix, windows)))]
    fn capture_local() -> Result<Self, RuntimeClockError> {
        Err(RuntimeClockError::LocalClockUnsupported)
    }

    fn from_datetime(datetime: DateTime<FixedOffset>) -> Result<Self, RuntimeClockError> {
        let offset_seconds = datetime.offset().fix().local_minus_utc();
        if offset_seconds % 60 != 0 {
            return Err(RuntimeClockError::SubMinuteUtcOffset(offset_seconds));
        }
        let year = datetime.year();
        if !(0..=9999).contains(&year) {
            return Err(RuntimeClockError::PdfYearOutOfRange(year));
        }
        Ok(Self {
            year,
            month: datetime.month(),
            day: datetime.day(),
            hour: datetime.hour(),
            minute: datetime.minute(),
            second: datetime.second(),
            utc_offset_minutes: offset_seconds / 60,
        })
    }

    pub(crate) const fn year(self) -> i32 {
        self.year
    }

    pub(crate) const fn month(self) -> i32 {
        self.month as i32
    }

    pub(crate) const fn day(self) -> i32 {
        self.day as i32
    }

    pub(crate) const fn tex_time(self) -> i32 {
        (self.hour * 60 + self.minute) as i32
    }

    pub(crate) fn transcript_month(self) -> &'static str {
        const MONTHS: [&str; 12] = [
            "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
        ];
        MONTHS[(self.month - 1) as usize]
    }

    pub(crate) fn pdf_creation_date(self) -> String {
        let offset = self.utc_offset_minutes;
        let sign = if offset < 0 { '-' } else { '+' };
        let absolute_offset = offset.unsigned_abs();
        format!(
            "D:{:04}{:02}{:02}{:02}{:02}{:02}{sign}{:02}'{:02}'",
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
            absolute_offset / 60,
            absolute_offset % 60,
        )
    }

    pub(crate) const fn hour(self) -> u32 {
        self.hour
    }

    pub(crate) const fn minute(self) -> u32 {
        self.minute
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RuntimeClockError {
    InvalidSourceDateEpoch(OsString),
    SourceDateEpochOutOfRange(i64),
    PdfYearOutOfRange(i32),
    SubMinuteUtcOffset(i32),
    #[cfg(not(any(unix, windows)))]
    LocalClockUnsupported,
}

impl Display for RuntimeClockError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSourceDateEpoch(value) => write!(
                formatter,
                "{SOURCE_DATE_EPOCH} must be an integral Unix timestamp, got {:?}",
                value.to_string_lossy()
            ),
            Self::SourceDateEpochOutOfRange(seconds) => write!(
                formatter,
                "{SOURCE_DATE_EPOCH}={seconds} is outside the supported calendar range"
            ),
            Self::PdfYearOutOfRange(year) => write!(
                formatter,
                "run year {year} cannot be represented by the four-digit PDF date format"
            ),
            Self::SubMinuteUtcOffset(seconds) => write!(
                formatter,
                "local UTC offset {seconds} seconds cannot be represented by the PDF date format"
            ),
            #[cfg(not(any(unix, windows)))]
            Self::LocalClockUnsupported => write!(
                formatter,
                "this target has no local-clock provider; set {SOURCE_DATE_EPOCH} or provide a host clock"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn 固定時刻(
        offset_seconds: i32,
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> RunDateTime {
        let offset = FixedOffset::east_opt(offset_seconds).unwrap();
        let datetime = offset
            .with_ymd_and_hms(year, month, day, hour, minute, second)
            .unwrap();
        RunDateTime::from_datetime(datetime).unwrap()
    }

    #[test]
    fn 日境界のtex_timeを分単位で固定する() {
        let before = 固定時刻(9 * 3600, 2024, 12, 31, 23, 59, 59);
        let after = 固定時刻(9 * 3600, 2025, 1, 1, 0, 0, 0);
        assert_eq!(before.tex_time(), 1439);
        assert_eq!(after.tex_time(), 0);
        assert_eq!(
            (before.year(), before.month(), before.day()),
            (2024, 12, 31)
        );
        assert_eq!((after.year(), after.month(), after.day()), (2025, 1, 1));
    }

    #[test]
    fn source_date_epochは閏日をutcで固定する() {
        let epoch = Utc
            .with_ymd_and_hms(2024, 2, 29, 12, 34, 56)
            .unwrap()
            .timestamp()
            .to_string();
        let clock = RunDateTime::from_source_date_epoch(OsStr::new(&epoch)).unwrap();
        assert_eq!((clock.year(), clock.month(), clock.day()), (2024, 2, 29));
        assert_eq!(clock.tex_time(), 12 * 60 + 34);
        assert_eq!(clock.pdf_creation_date(), "D:20240229123456+00'00'");
    }

    #[test]
    fn pdf日時は正負のtimezone_offsetを保つ() {
        let east = 固定時刻(9 * 3600 + 30 * 60, 2026, 8, 23, 1, 2, 3);
        let west = 固定時刻(-(3 * 3600 + 30 * 60), 2026, 8, 23, 1, 2, 3);
        assert_eq!(east.pdf_creation_date(), "D:20260823010203+09'30'");
        assert_eq!(west.pdf_creation_date(), "D:20260823010203-03'30'");
    }

    #[test]
    fn pdfで表せない秒単位offsetを拒む() {
        let offset = FixedOffset::east_opt(9 * 3600 + 1).unwrap();
        let datetime = offset.with_ymd_and_hms(2026, 8, 23, 1, 2, 3).unwrap();
        assert_eq!(
            RunDateTime::from_datetime(datetime),
            Err(RuntimeClockError::SubMinuteUtcOffset(9 * 3600 + 1))
        );
    }

    #[test]
    fn 不正なsource_date_epochから現在時刻へ戻らない() {
        for value in ["", " 0", "+0", "1.5", "12x"] {
            assert!(matches!(
                RunDateTime::from_source_date_epoch(OsStr::new(value)),
                Err(RuntimeClockError::InvalidSourceDateEpoch(_))
            ));
        }
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn local時計はosの現在時刻とoffsetを読む() {
        let before = chrono::Local::now().fixed_offset();
        let clock = RunDateTime::capture_local().unwrap();
        let after = chrono::Local::now().fixed_offset();
        let offset = FixedOffset::east_opt(clock.utc_offset_minutes * 60).unwrap();
        let captured = offset
            .with_ymd_and_hms(
                clock.year,
                clock.month,
                clock.day,
                clock.hour,
                clock.minute,
                clock.second,
            )
            .unwrap();
        assert!(
            captured.timestamp() >= before.timestamp() && captured.timestamp() <= after.timestamp(),
            "{before}..={after}"
        );
        assert!(
            captured.offset() == before.offset() || captured.offset() == after.offset(),
            "DST境界のどちらのoffsetでもない: {captured}"
        );
    }
}
