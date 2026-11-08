//! Home Assistant UTC offset and timezone refresh lifecycle.
use super::*;
impl Live {
    pub(super) fn tick_offset(&mut self, now: Instant, session: &mut Session, env: &mut impl Env) {
        // HA's render_template tracks now() and refreshes on each minute, including DST changes.
        if matches!(self.offset_stream, Stream::Idle)
            && self.offset_retry.is_none_or(|due| now >= due)
        {
            self.offset_stream = Stream::Pending(session.subscribe(
                now,
                Source::template("{{ now().utcoffset().total_seconds() | int }}"),
            ));
        }
        if let Stream::Pending(p) = &mut self.offset_stream
            && let Some(result) = p.poll()
        {
            match result {
                Ok(stream) => self.offset_stream = Stream::Active(stream),
                Err(e) => {
                    env.warn(&format!(
                        "Home Assistant UTC offset subscription failed: {e}"
                    ));
                    self.offset_stream = Stream::Idle;
                    self.offset_retry = Some(now + Duration::from_secs(30));
                }
            }
        }
        self.collect_offset(now);
    }
    fn collect_offset(&mut self, now: Instant) {
        if let Stream::Active(stream) = &mut self.offset_stream {
            for update in stream.drain() {
                match update {
                    Update::Event(TemplateUpdate::Rendered(value)) => {
                        self.offset = rendered_offset(&value);
                    }
                    Update::Event(_)
                    | Update::Status {
                        status:
                            SubscriptionStatus::Interrupted
                            | SubscriptionStatus::Restored
                            | SubscriptionStatus::Lagged { .. },
                        ..
                    } => self.offset = None,
                    _ => {}
                }
            }
            if stream.is_finished() {
                self.offset_stream = Stream::Idle;
                self.offset = None;
                self.offset_retry = Some(now + Duration::from_secs(30));
            }
        }
    }
    pub(super) fn tick_config(&mut self, now: Instant, session: &mut Session) {
        if let Some(reply) = &mut self.config
            && let Some(result) = reply.poll()
        {
            self.zone = result
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(raw.get()).ok())
                .and_then(|v| {
                    v["time_zone"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                });
            self.config = None;
            self.config_due =
                Some(now + Duration::from_secs(if self.zone.is_some() { 300 } else { 30 }));
        }
        if self.config.is_none() && self.config_due.is_none_or(|due| now >= due) {
            self.config = session
                .command(now, serde_json::json!({"type":"get_config"}))
                .ok();
        }
    }
}

fn rendered_offset(value: &Value) -> Option<FixedOffset> {
    value
        .as_i64()
        .or_else(|| value.as_str()?.parse().ok())
        .and_then(|n| i32::try_from(n).ok())
        .and_then(FixedOffset::east_opt)
}
