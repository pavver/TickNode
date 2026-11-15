//! Usage & Load statistics view.

use std::collections::HashMap;

use leptos::prelude::*;
use ticknode_shared::*;

use crate::i18n::{self, t};
use crate::state::use_editor;
use crate::ui::{IconButton, icons};

pub fn format_number(val: u64, locale: &str) -> String {
    let s = val.to_string();
    let sep = if locale == "uk" { ' ' } else { ',' };
    let mut out = String::new();
    let len = s.len();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

pub fn format_time_us(us: u64) -> String {
    if us >= 1_000_000 {
        format!("{:.2} s", us as f64 / 1_000_000.0)
    } else if us >= 1_000 {
        format!("{:.1} ms", us as f64 / 1_000.0)
    } else {
        format!("{us} µs")
    }
}

#[component]
pub fn UsageSection() -> impl IntoView {
    let ed = use_editor();
    let show_minutes = RwSignal::new(true);

    Effect::new(move |_| {
        ed.request_usage();
    });

    let locale = Memo::new(move |_| i18n::current_locale().code().to_string());

    let report_memo = Memo::new(move |_| ed.usage.get());

    let series = Memo::new(move |_| {
        let rep = report_memo.get();
        match rep {
            Some(r) => {
                if show_minutes.get() {
                    r.minutes
                } else {
                    r.hours
                }
            }
            None => Vec::new(),
        }
    });

    let has_data = Memo::new(move |_| {
        series.with(|s| !s.is_empty() && s.iter().any(|b| b.ticks > 0 || !b.plugins.is_empty()))
    });

    let current_tick_str = Memo::new(move |_| {
        let tick = report_memo.with(|r| r.as_ref().map_or(0, |x| x.current_tick));
        format_number(tick, &locale.get())
    });

    let totals = Memo::new(move |_| {
        let mut total_ticks: u64 = 0;
        let mut sum_tick_time_us: u64 = 0;
        let mut max_tick_us: u32 = 0;
        let mut total_spent: u64 = 0;
        let mut total_fuel: u64 = 0;
        let mut total_deferred: u64 = 0;

        series.with(|buckets| {
            for b in buckets {
                total_ticks = total_ticks.saturating_add(b.ticks as u64);
                sum_tick_time_us =
                    sum_tick_time_us.saturating_add((b.avg_tick_us as u64) * (b.ticks as u64));
                if b.max_tick_us > max_tick_us {
                    max_tick_us = b.max_tick_us;
                }
                total_spent = total_spent.saturating_add(b.total_spent);
                total_deferred = total_deferred.saturating_add(b.deferred_activations);
                for p in &b.plugins {
                    total_fuel = total_fuel.saturating_add(p.fuel);
                }
            }
        });

        let avg_tick_us = if total_ticks > 0 {
            sum_tick_time_us / total_ticks
        } else {
            0
        };

        (
            total_ticks,
            avg_tick_us,
            max_tick_us,
            total_spent,
            total_fuel,
            total_deferred,
        )
    });

    let sparkline_bars = Memo::new(move |_| {
        series.with(|buckets| {
            let max_val = buckets
                .iter()
                .map(|b| b.max_tick_us)
                .max()
                .unwrap_or(1)
                .max(1);
            buckets
                .iter()
                .map(|b| {
                    let pct = ((b.max_tick_us as f64 / max_val as f64) * 100.0).clamp(4.0, 100.0);
                    let info = format!(
                        "avg {} / max {}, {} ticks",
                        format_time_us(b.avg_tick_us as u64),
                        format_time_us(b.max_tick_us as u64),
                        b.ticks
                    );
                    (pct, info)
                })
                .collect::<Vec<_>>()
        })
    });

    let top_nodes = Memo::new(move |_| {
        let mut node_map: HashMap<NodeId, (String, Option<SheetId>, u64, u64)> = HashMap::new();
        let mut other_cost: u64 = 0;
        let mut other_exec: u64 = 0;

        series.with(|buckets| {
            for b in buckets {
                other_cost = other_cost.saturating_add(b.other_node_cost);
                other_exec = other_exec.saturating_add(b.other_node_executions);
                for n in &b.top_nodes {
                    let entry = node_map
                        .entry(n.node_id)
                        .or_insert_with(|| (n.type_name.clone(), n.sheet_id, 0, 0));
                    entry.2 = entry.2.saturating_add(n.cost);
                    entry.3 = entry.3.saturating_add(n.executions);
                }
            }
        });

        let mut list = node_map.into_values().collect::<Vec<_>>();
        list.sort_by(|a, b| b.2.cmp(&a.2));
        list.truncate(16);
        (list, other_cost, other_exec)
    });

    let top_plugins = Memo::new(move |_| {
        let mut plugin_map: HashMap<String, (u64, u64, u32)> = HashMap::new();
        series.with(|buckets| {
            for b in buckets {
                for p in &b.plugins {
                    let entry = plugin_map
                        .entry(p.plugin.clone())
                        .or_insert_with(|| (0, 0, 0));
                    entry.0 = entry.0.saturating_add(p.fuel);
                    entry.1 = entry.1.saturating_add(p.calls);
                    entry.2 = entry.2.saturating_add(p.traps);
                }
            }
        });
        let mut list = plugin_map.into_iter().collect::<Vec<_>>();
        list.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        list
    });

    let top_sheets = Memo::new(move |_| {
        let mut sheet_map: HashMap<Option<SheetId>, (u64, u64)> = HashMap::new();
        series.with(|buckets| {
            for b in buckets {
                for s in &b.sheets {
                    let entry = sheet_map.entry(s.sheet_id).or_insert_with(|| (0, 0));
                    entry.0 = entry.0.saturating_add(s.cost);
                    entry.1 = entry.1.saturating_add(s.executions);
                }
            }
        });
        let mut list = sheet_map.into_iter().collect::<Vec<_>>();
        list.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        list
    });

    let sheet_title = move |id: Option<SheetId>| match id {
        None => t!("usage-global"),
        Some(sid) => {
            let name = ed.sheets.with(|s| {
                s.iter()
                    .find(|x| x.info.id == sid)
                    .map(|x| x.info.title.clone())
            });
            name.unwrap_or_else(|| sid.to_string())
        }
    };

    view! {
        <div class="home-usage-section">
            <div class="usage-header-row">
                <div>
                    <span class="t-section">{move || t!("usage-title")}</span>
                    <span class="t-caption" style="margin-left: calc(var(--du) * 2);">
                        {move || format!("{} {}", t!("usage-current-tick"), current_tick_str.get())}
                    </span>
                </div>
                <div class="usage-header-actions">
                    <div class="usage-window-toggle">
                        <button
                            class="usage-window-btn ctl"
                            class:st-selected=move || show_minutes.get()
                            on:click=move |_| show_minutes.set(true)
                        >
                            {move || t!("usage-window-minutes")}
                        </button>
                        <button
                            class="usage-window-btn ctl"
                            class:st-selected=move || !show_minutes.get()
                            on:click=move |_| show_minutes.set(false)
                        >
                            {move || t!("usage-window-hours")}
                        </button>
                    </div>
                    <IconButton
                        icon=icons::REFRESH
                        title=Signal::derive(move || t!("usage-refresh"))
                        on_click=Callback::new(move |_| ed.request_usage())
                    />
                </div>
            </div>

            {move || (!has_data.get()).then(|| view! {
                <div class="sheets-empty">{move || t!("usage-empty")}</div>
            })}

            {move || has_data.get().then(|| {
                let (ticks, avg_us, max_us, work, fuel, _def) = totals.get();
                let loc = locale.get();
                view! {
                    <div class="usage-summary-grid">
                        <div class="usage-summary-card">
                            <span class="usage-card-label">{move || t!("usage-avg-tick")}</span>
                            <span class="usage-card-val">{format_time_us(avg_us)}</span>
                            <span class="usage-card-sub">
                                {format!("{}: {}", t!("usage-max-tick"), format_time_us(max_us as u64))}
                            </span>
                        </div>
                        <div class="usage-summary-card">
                            <span class="usage-card-label">{move || t!("usage-ticks")}</span>
                            <span class="usage-card-val">{format_number(ticks, &loc)}</span>
                            <span class="usage-card-sub">
                                {format!("{}: {}", t!("usage-spent-work"), format_number(work, &loc))}
                            </span>
                        </div>
                        <div class="usage-summary-card">
                            <span class="usage-card-label">{move || t!("usage-fuel")}</span>
                            <span class="usage-card-val">{format_number(fuel, &loc)}</span>
                            <span class="usage-card-sub">{move || t!("usage-plugins")}</span>
                        </div>
                    </div>

                    <div class="usage-chart-card">
                        <span class="usage-chart-title">{move || t!("usage-max-tick")}</span>
                        <div class="usage-sparkline-container">
                            <For each=move || sparkline_bars.get() key=|(_, info)| info.clone() let:bar>
                                <div
                                    class="usage-spark-bar"
                                    style=format!("height: {}%", bar.0)
                                    title=bar.1
                                />
                            </For>
                        </div>
                    </div>

                    <div class="usage-table-card">
                        <span class="usage-chart-title">{move || t!("usage-top-nodes")}</span>
                        <table class="usage-table">
                            <thead>
                                <tr>
                                    <th>{move || t!("usage-node-name")}</th>
                                    <th>{move || t!("usage-sheet")}</th>
                                    <th class="col-num">{move || t!("usage-cost")}</th>
                                    <th class="col-num">{move || t!("usage-executions")}</th>
                                </tr>
                            </thead>
                            <tbody>
                                <For each=move || top_nodes.get().0 key=|(name, sheet, c, e)| format!("{name}-{sheet:?}-{c}-{e}") let:node>
                                    <tr>
                                        <td>{node.0}</td>
                                        <td>{sheet_title(node.1)}</td>
                                        <td class="col-num">{format_number(node.2, &locale.get())}</td>
                                        <td class="col-num">{format_number(node.3, &locale.get())}</td>
                                    </tr>
                                </For>
                                {move || {
                                    let (_, oc, oe) = top_nodes.get();
                                    (oc > 0).then(|| view! {
                                        <tr>
                                            <td><em>{move || t!("usage-other-nodes")}</em></td>
                                            <td>{"-"}</td>
                                            <td class="col-num">{format_number(oc, &locale.get())}</td>
                                            <td class="col-num">{format_number(oe, &locale.get())}</td>
                                        </tr>
                                    })
                                }}
                            </tbody>
                        </table>
                    </div>

                    <div class="usage-table-card">
                        <span class="usage-chart-title">{move || t!("usage-plugins")}</span>
                        <table class="usage-table">
                            <thead>
                                <tr>
                                    <th>{move || t!("usage-plugin-name")}</th>
                                    <th class="col-num">{move || t!("usage-fuel")}</th>
                                    <th class="col-num">{move || t!("usage-calls")}</th>
                                    <th class="col-num">{move || t!("usage-traps")}</th>
                                </tr>
                            </thead>
                            <tbody>
                                <For each=move || top_plugins.get() key=|(name, _)| name.clone() let:plugin>
                                    <tr>
                                        <td>{plugin.0}</td>
                                        <td class="col-num">{format_number(plugin.1.0, &locale.get())}</td>
                                        <td class="col-num">{format_number(plugin.1.1, &locale.get())}</td>
                                        <td class="col-num">{format_number(plugin.1.2 as u64, &locale.get())}</td>
                                    </tr>
                                </For>
                            </tbody>
                        </table>
                    </div>

                    <div class="usage-table-card">
                        <span class="usage-chart-title">{move || t!("usage-sheets-title")}</span>
                        <table class="usage-table">
                            <thead>
                                <tr>
                                    <th>{move || t!("usage-sheet")}</th>
                                    <th class="col-num">{move || t!("usage-cost")}</th>
                                    <th class="col-num">{move || t!("usage-executions")}</th>
                                </tr>
                            </thead>
                            <tbody>
                                <For each=move || top_sheets.get() key=|(id, _)| format!("{id:?}") let:sheet>
                                    <tr>
                                        <td>{sheet_title(sheet.0)}</td>
                                        <td class="col-num">{format_number(sheet.1.0, &locale.get())}</td>
                                        <td class="col-num">{format_number(sheet.1.1, &locale.get())}</td>
                                    </tr>
                                </For>
                            </tbody>
                        </table>
                    </div>
                }
            })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0, "en"), "0");
        assert_eq!(format_number(123, "en"), "123");
        assert_eq!(format_number(1234, "en"), "1,234");
        assert_eq!(format_number(1234567, "en"), "1,234,567");
        assert_eq!(format_number(1234567, "uk"), "1 234 567");
    }

    #[test]
    fn test_format_time_us() {
        assert_eq!(format_time_us(42), "42 µs");
        assert_eq!(format_time_us(1500), "1.5 ms");
        assert_eq!(format_time_us(2_500_000), "2.50 s");
    }
}
