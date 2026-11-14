//! Date and time pattern tokens, greedy lexing, and English fallback names.

pub const ENGLISH_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

pub const ENGLISH_MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

pub const ENGLISH_WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

pub const ENGLISH_WEEKDAYS_SHORT: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Year4,
    Year2,
    MonthName,
    MonthShort,
    Month2,
    Month1,
    Day2,
    Day1,
    WeekdayFull,
    WeekdayShort,
    OffsetCol,
    OffsetNoCol,
    Hour24Two,
    Hour24One,
    Hour12Two,
    Hour12One,
    Minute2,
    Minute1,
    Second2,
    Second1,
    Millis,
    AmPmUpper,
    AmPmLower,
}

impl Token {
    pub fn is_date_token(self) -> bool {
        matches!(
            self,
            Self::Year4
                | Self::Year2
                | Self::MonthName
                | Self::MonthShort
                | Self::Month2
                | Self::Month1
                | Self::Day2
                | Self::Day1
                | Self::WeekdayFull
                | Self::WeekdayShort
                | Self::OffsetCol
                | Self::OffsetNoCol
        )
    }

    pub fn is_day_token(self) -> bool {
        matches!(self, Self::Day2 | Self::Day1)
    }
}

pub enum PatternItem {
    Token(Token),
    Literal(String),
}

fn match_token(s: &str) -> Option<(Token, usize)> {
    if s.starts_with("YYYY") {
        return Some((Token::Year4, 4));
    }
    if s.starts_with("MMMM") {
        return Some((Token::MonthName, 4));
    }
    if s.starts_with("dddd") {
        return Some((Token::WeekdayFull, 4));
    }
    if s.starts_with("MMM") {
        return Some((Token::MonthShort, 3));
    }
    if s.starts_with("ddd") {
        return Some((Token::WeekdayShort, 3));
    }
    if s.starts_with("SSS") {
        return Some((Token::Millis, 3));
    }
    if s.starts_with("YY") {
        return Some((Token::Year2, 2));
    }
    if s.starts_with("MM") {
        return Some((Token::Month2, 2));
    }
    if s.starts_with("DD") {
        return Some((Token::Day2, 2));
    }
    if s.starts_with("HH") {
        return Some((Token::Hour24Two, 2));
    }
    if s.starts_with("hh") {
        return Some((Token::Hour12Two, 2));
    }
    if s.starts_with("mm") {
        return Some((Token::Minute2, 2));
    }
    if s.starts_with("ss") {
        return Some((Token::Second2, 2));
    }
    if s.starts_with("ZZ") {
        return Some((Token::OffsetNoCol, 2));
    }
    if s.starts_with('M') {
        return Some((Token::Month1, 1));
    }
    if s.starts_with('D') {
        return Some((Token::Day1, 1));
    }
    if s.starts_with('H') {
        return Some((Token::Hour24One, 1));
    }
    if s.starts_with('h') {
        return Some((Token::Hour12One, 1));
    }
    if s.starts_with('m') {
        return Some((Token::Minute1, 1));
    }
    if s.starts_with('s') {
        return Some((Token::Second1, 1));
    }
    if s.starts_with('A') {
        return Some((Token::AmPmUpper, 1));
    }
    if s.starts_with('a') {
        return Some((Token::AmPmLower, 1));
    }
    if s.starts_with('Z') {
        return Some((Token::OffsetCol, 1));
    }
    None
}

pub fn parse_pattern(mut pattern: &str) -> Vec<PatternItem> {
    let mut items = Vec::new();
    while !pattern.is_empty() {
        if pattern.starts_with('[') {
            if let Some(end) = pattern.find(']') {
                let literal = &pattern[1..end];
                if !literal.is_empty() {
                    items.push(PatternItem::Literal(literal.to_string()));
                }
                pattern = &pattern[end + 1..];
                continue;
            } else {
                let literal = &pattern[1..];
                if !literal.is_empty() {
                    items.push(PatternItem::Literal(literal.to_string()));
                }
                break;
            }
        }

        if let Some((token, len)) = match_token(pattern) {
            items.push(PatternItem::Token(token));
            pattern = &pattern[len..];
        } else {
            let ch = pattern.chars().next().unwrap_or(' ');
            items.push(PatternItem::Literal(ch.to_string()));
            pattern = &pattern[ch.len_utf8()..];
        }
    }
    items
}
