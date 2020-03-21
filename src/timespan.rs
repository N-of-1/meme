use chrono::{DateTime, Duration, Local};

pub trait Span {
    fn new() -> Self;

    fn set_previous(&mut self, previous_span: SpanStruct);

    fn set_duration(&mut self, duration: Duration);
}

enum TimeSpanDuration {
    FixedDuration { duration: Duration },
    Containing { items: Vec<TimeSpanDuration>},
}

enum TimeSpan<'a> {
    Head {
        time: DateTime<Local>,
    },
    AfterPrevious {
        previous: &'a TimeSpan<'a>,
        duration: TimeSpanDuration,
    },
}

pub struct SpanStruct<'a> {
    start_time: TimeSpan<'a>,
}
