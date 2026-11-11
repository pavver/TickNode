//! Authentication, single-flight long polling and bounded auxiliary requests.
use super::*;

impl Hub {
    pub(super) fn start(&mut self, request: Request, body: Value, env: &mut impl Env, now: u64) {
        let method = match request {
            Request::Me => "getMe",
            Request::Poll => "getUpdates",
            Request::Chat(_) => "getChat",
            Request::Answer => "answerCallbackQuery",
        };
        match env.http(api::request(&self.base, &self.token, method, body)) {
            Ok(id) => {
                self.pending.insert(id, request);
            }
            Err(_) => {
                env.log("Could not start Telegram HTTP request");
                self.retry(now, None);
            }
        }
    }
    fn retry(&mut self, now: u64, after: Option<u64>) {
        self.next = now.saturating_add(after.unwrap_or(self.backoff).max(1));
        self.backoff = (self.backoff * 2).clamp(1, 30);
    }
    fn completed(
        &mut self,
        request: Request,
        result: Result<TaskOutput, String>,
        now: u64,
        env: &mut impl Env,
    ) {
        match api::response(result) {
            Ok(value) => match request {
                Request::Me => {
                    if let Some(username) = value["username"].as_str() {
                        self.username = Some(username.into());
                        env.log(&format!("Telegram authenticated as @{username}"));
                        self.backoff = 1;
                    } else {
                        env.log("Telegram getMe returned no bot username");
                        self.retry(now, None);
                    }
                }
                Request::Poll => {
                    self.backoff = 1;
                    if value.is_array() {
                        self.updates(value, now, env);
                    } else {
                        env.log("Invalid Telegram getUpdates result");
                        self.retry(now, None);
                    }
                }
                Request::Chat(_) => {
                    if let Ok(chat) = serde_json::from_value(value) {
                        self.directory.chat(&chat, now);
                    }
                }
                Request::Answer => {}
            },
            Err(mut failure) => {
                failure.description = failure.description.replace(&self.token, "[redacted]");
                if failure.code == 401 {
                    self.unauthorized();
                } else {
                    if failure.code == 409 {
                        env.log("Telegram 409 Conflict: remove the bot webhook (deleteWebhook) and stop other getUpdates instances");
                    } else {
                        env.log(&format!(
                            "Telegram {method} failed: {}",
                            failure.description,
                            method = match request {
                                Request::Me => "getMe",
                                Request::Poll => "getUpdates",
                                Request::Chat(_) => "getChat",
                                Request::Answer => "answerCallbackQuery",
                            }
                        ));
                    }
                    if matches!(request, Request::Me | Request::Poll) {
                        self.retry(now, failure.retry_after);
                    }
                }
            }
        }
    }
    pub fn tick(
        &mut self,
        now: u64,
        events: Vec<PluginEvent>,
        env: &mut impl Env,
    ) -> Vec<Emission> {
        for event in events {
            if let PluginEvent::HttpDone { id, result } = event
                && let Some(request) = self.pending.remove(&id)
            {
                self.completed(request, result, now, env);
            }
        }
        if std::mem::take(&mut self.authentication_error) {
            env.settings_error(&validation::error(
                "unauthorized",
                "Telegram rejected the bot token; update the plugin settings",
            ));
        }
        // Never confirm an offset with Telegram until it is durably recorded locally.
        if self.offset_dirty {
            if env
                .write("offset.json", self.offset.to_string().as_bytes())
                .is_ok()
            {
                self.offset_dirty = false;
            } else {
                env.log("Could not persist Telegram offset; polling paused until storage recovers");
            }
        }
        self.directory.maintain(now, self.ttl);
        self.directory.publish(env);
        self.directory.flush(now, env);
        self.lookup_days.retain(|_, day| *day >= now / 86400);
        if !self.stopped && now >= self.next {
            if self.username.is_none() {
                if !self.pending.values().any(|r| matches!(r, Request::Me)) {
                    self.start(Request::Me, json!({}), env, now);
                }
            } else {
                if !self.offset_dirty && !self.pending.values().any(|r| matches!(r, Request::Poll))
                {
                    self.start(Request::Poll, json!({"offset":self.offset,"timeout":50,"limit":100,
                        "allowed_updates":["message","edited_message","callback_query","my_chat_member"]}), env, now);
                }
                // Leave room in the host's eight plugin HTTP slots for the poll and authentication.
                while self.pending.len() < 6 {
                    let Some((request, body)) = self.aux.pop_front() else {
                        break;
                    };
                    if let Request::Chat(chat) = request {
                        if self.directory.chats.contains_key(&chat) || !self.permits(chat) {
                            continue;
                        }
                        self.lookup_days.insert(chat, now / 86400);
                        if env
                            .write(
                                "lookups.json",
                                &serde_json::to_vec(&self.lookup_days).unwrap(),
                            )
                            .is_err()
                        {
                            env.log(
                                "Could not persist Telegram getChat lookup day; request skipped",
                            );
                            continue;
                        }
                    }
                    self.start(request, body, env, now);
                }
            }
        }
        self.watches
            .iter_mut()
            .filter_map(|(node, watch)| {
                if watch.overflow {
                    env.log("Telegram trigger queue full; oldest event discarded");
                    watch.overflow = false;
                }
                Some(Emission::new(*node, watch.queue.pop_front()?))
            })
            .collect()
    }
}
