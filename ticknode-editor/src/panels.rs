//! Бічні панелі (палітра, інспектор) і консоль.

use std::collections::BTreeMap;

use leptos::html;
use leptos::prelude::*;
use ticknode_shared::*;
use wasm_bindgen::JsCast;
use web_sys::PointerEvent;

use crate::canvas::{CANVAS_ID, view_center};
use crate::clipboard;
use crate::categories::{category_color, category_icon, category_label, category_rank};
use crate::geometry::{DU, node_width, pin_color, type_color};
use crate::state::{Editor, LogKind, PaletteDrag, Selection, Tooltip, use_editor};
use crate::ui::*;

/// Ширина підказки «інфо», px.
const TOOLTIP_WIDTH: f64 = 80.0 * DU;
/// Зсув (px), після якого натискання на пункт палітри стає перетягуванням.
const PALETTE_DRAG_THRESHOLD: f64 = 4.0;
/// Довші значення в інспекторі обрізаються (повне — у підказці).
const MAX_VALUE_CHARS: usize = 400;


pub fn property_label(name: &str) -> String {
    match name {
        "value" => "Значення",
        "var_name" => "Змінна",
        "expression" => "Формула",
        "prefix" => "Префікс",
        "label" => "Мітка",
        "element_type" => "Тип елементів",
        "field_type" => "Тип поля",
        "fields_list" => "Поля (name:type, …)",
        "interval_ticks" => "Інтервал, тактів",
        "all_matches" => "Усі збіги",
        "cases" => "Випадки",
        other => other,
    }
    .to_string()
}

/// Значення для показу в інспекторі: рядки в лапках, щоб відрізнити `"5"` від `5`.
pub fn display_value(value: &DataValue) -> String {
    match value {
        DataValue::String(s) => format!("\"{s}\""),
        DataValue::Number(NumberValue::Int(i)) => i.to_string(),
        DataValue::Number(NumberValue::Float(f)) => f.to_string(),
        DataValue::Bool(b) => b.to_string(),
        DataValue::Time(t) => t.format("%H:%M:%S").to_string(),
        DataValue::DateTime(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        DataValue::Duration(d) => text::format_duration(*d),
        DataValue::Color(c) => format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a),
        DataValue::List { elements, .. } => {
            format!("[{}]", elements.iter().map(display_value).collect::<Vec<_>>().join(", "))
        }
        DataValue::Object { fields, .. } => {
            let fields: Vec<_> = fields.iter().map(|(k, f)| format!("{k}: {}", display_value(&f.value))).collect();
            format!("{{{}}}", fields.join(", "))
        }
    }
}

fn truncate(text: String, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text,
    }
}

/// Українська форма множини: 1 секунду, 2 секунди, 5 секунд.
fn plural<'a>(n: u64, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    match (n % 10, n % 100) {
        (1, r) if r != 11 => one,
        (2..=4, r) if !(12..=14).contains(&r) => few,
        _ => many,
    }
}

pub fn relative_time(ms: f64) -> String {
    let secs = (ms.max(0.0) / 1000.0).floor() as u64;
    let (n, unit) = match secs {
        0 => return "щойно".to_string(),
        s if s < 60 => (s, plural(s, "секунду", "секунди", "секунд")),
        s if s < 3600 => (s / 60, plural(s / 60, "хвилину", "хвилини", "хвилин")),
        s if s < 86_400 => (s / 3600, plural(s / 3600, "годину", "години", "годин")),
        s => (s / 86_400, plural(s / 86_400, "день", "дні", "днів")),
    };
    format!("{n} {unit} тому")
}

// --- Палітра ---

#[component]
pub fn Palette() -> impl IntoView {
    let ed = use_editor();
    let query = RwSignal::new(String::new());
    let groups = Memo::new(move |_| {
        let q = query.get().trim().to_lowercase();
        let mut groups: BTreeMap<(usize, String), Vec<NodeDefinition>> = BTreeMap::new();
        for def in ed.definitions.get() {
            let haystack = format!("{} {} {} {}", def.title, def.type_name, def.description, category_label(&def.category)).to_lowercase();
            if q.is_empty() || haystack.contains(&q) {
                groups.entry((category_rank(&def.category), def.category.clone())).or_default().push(def);
            }
        }
        groups.into_iter().map(|((_, category), defs)| (category, defs)).collect::<Vec<_>>()
    });
    // Категорія, до якої прокручено список (підсвічується на смузі ліворуч)
    let active = RwSignal::new(None::<String>);
    let list = NodeRef::<html::Div>::new();

    let update_active = move || {
        let Some(list) = list.get_untracked() else { return };
        let top = list.scroll_top() as f64 + 2.0 * DU;
        let sections = list.query_selector_all("[data-category]").ok();
        let mut current = None;
        for i in 0..sections.as_ref().map_or(0, |s| s.length()) {
            let Some(el) = sections.as_ref().and_then(|s| s.item(i)).and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok()) else { continue };
            if (el.offset_top() as f64) <= top || current.is_none() {
                current = el.get_attribute("data-category");
            }
        }
        active.set(current);
    };
    Effect::new(move |_| {
        groups.track();
        request_animation_frame(update_active);
    });
    let scroll_to = move |category: String| {
        let Some(list) = list.get_untracked() else { return };
        let section = list.query_selector_all("[data-category]").ok().and_then(|s| {
            (0..s.length()).filter_map(|i| s.item(i)?.dyn_into::<web_sys::HtmlElement>().ok())
                .find(|el| el.get_attribute("data-category").as_deref() == Some(category.as_str()))
        });
        if let Some(section) = section {
            list.set_scroll_top(section.offset_top());
            active.set(Some(category));
        }
    };

    let add = move |type_name: String| {
        let (cx, cy) = view_center(ed);
        // Невеликий зсув, щоб кілька нових нод не лягали одна на одну
        let shift = (ed.nodes.with_untracked(|n| n.len()) % 6) as f64 * 4.0 * DU;
        let position = Position { x: (cx - node_width(&type_name) / 2.0 + shift) as f32, y: (cy - 10.0 * DU + shift) as f32 };
        ed.send(ClientMessage::AddNode { type_name, position });
        if ed.narrow.get_untracked() {
            ed.show_palette.set(false);
        }
    };

    view! {
        <aside
            class="panel left palette"
            class:hidden=move || !ed.show_palette.get()
            class:dragging-out=move || ed.palette_drag.with(|d| d.as_ref().is_some_and(|d| d.active))
        >
            <nav class="palette-rail">
                {move || groups.get().into_iter().map(|(category, _)| {
                    let key = category.clone();
                    let is_active = move || active.with(|a| a.as_deref() == Some(key.as_str()));
                    let target = category.clone();
                    view! {
                        <button
                            class="rail-item"
                            class:active=is_active
                            style=format!("--cat-color:{}", category_color(&category))
                            title=category.clone()
                            on:click=move |_| scroll_to(target.clone())
                        >
                            <span class="rail-swatch"><Icon path=category_icon(&category) /></span>
                            <span class="rail-label">{category_label(&category)}</span>
                        </button>
                    }
                }).collect_view()}
            </nav>
            <div class="palette-main">
                <div class="panel-section palette-search">
                    <TextField
                        value=Signal::derive(String::new)
                        placeholder="Пошук нод"
                        icon=icons::SEARCH
                        on_input=Callback::new(move |v| query.set(v))
                    />
                </div>
                <div class="palette-list" node_ref=list on:scroll=move |_| update_active()>
                    {move || {
                        let groups = groups.get();
                        if groups.is_empty() {
                            return view! { <div class="empty">"Нічого не знайдено"</div> }.into_any();
                        }
                        groups.into_iter().map(|(category, defs)| {
                            let color = category_color(&category);
                            view! {
                                <section class="palette-group" data-category=category.clone() style=format!("--cat-color:{color}")>
                                    <div class="palette-group-title">
                                        <span class="group-icon"><Icon path=category_icon(&category) /></span>
                                        {category_label(&category)}
                                    </div>
                                    {defs.into_iter().map(|def| view! { <PaletteBlock def=def color=color add=add /> }).collect_view()}
                                </section>
                            }
                        }).collect_view().into_any()
                    }}
                </div>
            </div>
        </aside>
    }
}

/// Блок палітри: назва, піни (ліворуч входи, праворуч виходи) й опис. Клік — нода в центр,
/// перетягування — нода туди, де відпустили.
#[component]
fn PaletteBlock(def: NodeDefinition, color: &'static str, add: impl Fn(String) + Copy + 'static) -> impl IntoView {
    let ed = use_editor();
    let (type_name, title) = (def.type_name.clone(), def.title.clone());
    let start_drag = move |ev: PointerEvent| {
        if ev.button() != 0 {
            return;
        }
        if let Some(el) = ev.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
            let _ = el.set_pointer_capture(ev.pointer_id());
        }
        let at = (ev.client_x() as f64, ev.client_y() as f64);
        ed.palette_drag.set(Some(PaletteDrag { type_name: type_name.clone(), title: title.clone(), color, start: at, at, active: false }));
    };
    let pin_chips = |dir: PinDirection| {
        def.pins.iter().filter(|p| p.direction == dir).map(|p| {
            let exec = p.pin_type == PinType::Exec;
            let type_name = match &p.pin_type {
                PinType::Exec => "exec".to_string(),
                PinType::Data(t) => ed.type_title(t),
            };
            view! {
                <span class="block-pin" title=format!("{}: {}", p.name, type_name)>
                    <span class="socket connected" class:exec=exec style=format!("--pin-color:{}", ed.types.with_untracked(|types| pin_color(types, &p.pin_type)))></span>
                    {(!exec).then(|| p.name.clone())}
                </span>
            }
        }).collect_view()
    };
    view! {
        <div
            class="palette-block palette-item"
            on:pointerdown=start_drag
            on:pointermove=move |ev: PointerEvent| move_palette_drag(ed, &ev)
            on:pointerup=move |ev: PointerEvent| match ed.palette_drag.try_update(Option::take).flatten() {
                Some(drag) if drag.active => drop_on_canvas(ed, &drag, &ev),
                // Клік без перетягування — додати в центр
                Some(drag) => add(drag.type_name),
                None => {}
            }
            on:pointercancel=move |_| ed.palette_drag.set(None)
        >
            <div class="block-head">
                <span class="block-title ellipsis">{def.title.clone()}</span>
                <InfoButton def=def.clone() />
            </div>
            <div class="block-pins">
                <span class="block-pins-side in">{pin_chips(PinDirection::Input)}</span>
                <span class="block-pins-side out">{pin_chips(PinDirection::Output)}</span>
            </div>
            <div class="block-desc">{def.description.clone()}</div>
        </div>
    }
}

fn move_palette_drag(ed: Editor, ev: &PointerEvent) {
    ed.palette_drag.update(|drag| {
        if let Some(drag) = drag {
            drag.at = (ev.client_x() as f64, ev.client_y() as f64);
            let (dx, dy) = (drag.at.0 - drag.start.0, drag.at.1 - drag.start.1);
            drag.active |= (dx * dx + dy * dy).sqrt() > PALETTE_DRAG_THRESHOLD;
        }
    });
}

/// Нода з'являється там, де відпустили кнопку (курсор — над заголовком ноди).
fn drop_on_canvas(ed: Editor, drag: &PaletteDrag, ev: &PointerEvent) {
    let (x, y) = (ev.client_x(), ev.client_y());
    let over_canvas = document()
        .element_from_point(x as f32, y as f32)
        .and_then(|el| el.closest(&format!("#{CANVAS_ID}")).ok().flatten());
    let Some(canvas) = over_canvas else { return };
    let r = canvas.get_bounding_client_rect();
    let (wx, wy) = ed.view.get_untracked().to_world(x as f64 - r.left(), y as f64 - r.top());
    let snap = |v: f64| ((v / DU).round() * DU) as f32;
    let position = Position { x: snap(wx - 8.0 * DU), y: snap(wy - 4.0 * DU) };
    ed.send(ClientMessage::AddNode { type_name: drag.type_name.clone(), position });
    if ed.narrow.get_untracked() {
        ed.show_palette.set(false);
    }
}

/// Кнопка «інфо»: при наведенні (або дотику) показує детальне пояснення ноди.
#[component]
fn InfoButton(def: NodeDefinition) -> impl IntoView {
    let ed = use_editor();
    let text = if def.help.trim().is_empty() { def.description.clone() } else { def.help.clone() };
    let title = def.title.clone();
    let show = move |ev: &web_sys::MouseEvent| {
        let Some(el) = ev.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else { return };
        let r = el.get_bounding_client_rect();
        ed.tooltip.set(Some(Tooltip { title: title.clone(), text: text.clone(), anchor: (r.left(), r.top(), r.right(), r.bottom()) }));
    };
    let show_hover = show.clone();
    view! {
        <span
            class="info-btn"
            role="button"
            aria-label="Як працює нода"
            on:mouseenter=move |ev| show_hover(&ev)
            on:mouseleave=move |_| ed.tooltip.set(None)
            // Не починати перетягування блоку палітри
            on:pointerdown=|ev: PointerEvent| ev.stop_propagation()
            // На сенсорі наведення немає: дотик показує підказку, дотик деінде — ховає (див. App)
            on:click=move |ev: web_sys::MouseEvent| {
                ev.stop_propagation();
                show(&ev);
            }
        >
            <Icon path=icons::INFO />
        </span>
    }
}

/// Підказка «інфо» поверх усього: праворуч від кнопки, а якщо не влазить — ліворуч.
#[component]
pub fn TooltipLayer() -> impl IntoView {
    let ed = use_editor();
    move || {
        let tip = ed.tooltip.get()?;
        let (left, top, right, bottom) = tip.anchor;
        let (vw, vh) = (
            window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0),
            window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0),
        );
        let width = TOOLTIP_WIDTH.min(vw - 4.0 * DU);
        let gap = 2.0 * DU;
        let x = if right + gap + width <= vw { right + gap } else { (left - gap - width).max(2.0 * DU) };
        // У нижній половині екрана — вирівнюємо за низом кнопки, щоб підказка не вилазила за край
        let y = if top < vh / 2.0 { format!("top:{top}px") } else { format!("bottom:{}px", vh - bottom) };
        Some(view! {
            <div class="tooltip" style=format!("left:{x}px;{y};width:{width}px")>
                <div class="tooltip-title">{tip.title}</div>
                <div class="tooltip-text">{tip.text}</div>
            </div>
        })
    }
}

/// Привид ноди під курсором під час перетягування з палітри.
#[component]
pub fn PaletteGhost() -> impl IntoView {
    let ed = use_editor();
    move || {
        ed.palette_drag.get().filter(|d| d.active).map(|d| view! {
            <div class="palette-ghost" style=format!("left:{}px;top:{}px;--cat-color:{}", d.at.0, d.at.1, d.color)>
                <span class="node-title ellipsis">{d.title}</span>
            </div>
        })
    }
}

// --- Інспектор ---

#[component]
pub fn Inspector() -> impl IntoView {
    let ed = use_editor();
    // Панель плагінів займає місце інспектора
    let hidden = move || ed.show_plugins.get() || (ed.narrow.get() && ed.selection.get().is_none());
    view! {
        <aside class="panel right" class:hidden=hidden>
            {move || match ed.selection.get() {
                Some(Selection::Nodes(ids)) if ids.len() == 1 => {
                    let id = *ids.first().expect("одна нода");
                    view! { <NodeInspector id=id /> }.into_any()
                }
                Some(Selection::Nodes(ids)) => view! { <GroupInspector count=ids.len() /> }.into_any(),
                Some(Selection::Exec(c)) => view! { <ConnectionInspector conn=c exec=true /> }.into_any(),
                Some(Selection::Data(c)) => view! { <ConnectionInspector conn=c exec=false /> }.into_any(),
                None => view! { <GraphOverview /> }.into_any(),
            }}
        </aside>
    }
}

#[component]
fn GraphOverview() -> impl IntoView {
    let ed = use_editor();
    let stats = move || {
        let nodes = ed.nodes.with(|n| n.len());
        let conns = ed.exec.with(|e| e.len()) + ed.data.with(|d| d.len());
        format!("{} нод · {} з'єднань", nodes, conns)
    };
    let hints = [
        "Палітра: клік — нода в центр, перетягування — нода на місце",
        "Тягніть від піна до піна, щоб з'єднати; підключений вхід даних — щоб перенести дріт",
        "Рамка ЛКМ: зліва направо — ноди повністю всередині, справа наліво — всі, яких торкнулась",
        "Shift/Ctrl — додати до виділення; виділені ноди тягнуться разом",
        "Ctrl+C / Ctrl+X / Ctrl+V — буфер, Ctrl+D — дублювати, Ctrl+A — усе",
        "Вставка їде за курсором: ЛКМ — поставити, ПКМ або Esc — скасувати, середня кнопка — зсув",
        "Delete — видалити виділене, Esc — зняти виділення",
        "Середня кнопка — зсув, колесо — масштаб, F — весь граф; на сенсорі — палець і два пальці",
    ];
    view! {
        <div class="section-title">"Граф"</div>
        <div class="panel-section"><span class="t-row">{stats}</span></div>
        <div class="section-title">"Керування"</div>
        <div class="panel-section">
            {hints.into_iter().map(|h| view! { <span class="t-body-small">{h}</span> }).collect_view()}
        </div>
    }
}

#[component]
fn NodeInspector(id: NodeId) -> impl IntoView {
    let ed = use_editor();
    let Some(node) = ed.node(id) else {
        return view! { <div class="empty">"Ноду не знайдено"</div> }.into_any();
    };
    let def = Memo::new(move |_| node.instance.with(|i| i.definition.clone()));

    view! {
        <div class="panel-section">
            <div class="inspector-head">
                <span class="t-row ellipsis">{move || def.with(|d| d.title.clone())}</span>
                {move || view! { <InfoButton def=def.get() /> }}
            </div>
            <span class="t-caption">{move || def.with(|d| format!("{} · {}", category_label(&d.category), d.type_name))}</span>
            <span class="t-body-small">{move || def.with(|d| d.description.clone())}</span>
        </div>
        // Властивості, значення входів, умови й запуск — на самій ноді
        <ExecutionInfo id=id />
        <div class="section-title">"Дії"</div>
        <div class="panel-section">
            <ActionButton text="Дублювати (Ctrl+D)" icon=icons::DUPLICATE full=true on_click=Callback::new(move |_| clipboard::duplicate(ed)) />
            <ActionButton
                text="Видалити ноду"
                icon=icons::DELETE
                danger=true
                full=true
                on_click=Callback::new(move |_| ed.send(ClientMessage::RemoveNode(id)))
            />
        </div>
    }
    .into_any()
}

/// Коли нода виконувалась і з якими значеннями на пінах.
#[component]
fn ExecutionInfo(id: NodeId) -> impl IntoView {
    let ed = use_editor();
    let Some(node) = ed.node(id) else { return ().into_any() };
    let def = Memo::new(move |_| node.instance.with(|i| i.definition.clone()));
    let when = move || {
        let at = node.last_exec.get();
        if at.is_finite() {
            format!("Виконано {}", relative_time(ed.now.get() - at))
        } else {
            "Ще не виконувалась".to_string()
        }
    };
    let tick = move || node.trace.with(|t| t.as_ref().map(|t| format!("такт {}", t.tick)));

    let rows = move |dir: PinDirection| {
        let pins: Vec<PinDefinition> = def.with(|d| d.pins.iter().filter(|p| p.direction == dir).cloned().collect());
        pins.into_iter().map(|p| {
            let pin = p.id;
            let exec = p.pin_type == PinType::Exec;
            // (текст, приглушений, підказка)
            let value = move || -> (String, bool, String) {
                let connected = match dir {
                    PinDirection::Input => ed.is_input_connected(id, pin),
                    PinDirection::Output => ed.is_output_connected(id, pin),
                };
                node.trace.with(|trace| {
                    let Some(trace) = trace else {
                        return ("—".into(), true, "Нода ще не виконувалась".into());
                    };
                    match (dir, exec) {
                        (PinDirection::Input, true) => (String::new(), true, String::new()),
                        (PinDirection::Output, true) if trace.fired.contains(&pin) => ("▶ спрацював".into(), false, String::new()),
                        (PinDirection::Output, true) => ("—".into(), true, "Не спрацював в останньому виконанні".into()),
                        (PinDirection::Input, false) => match trace.inputs.iter().find(|(p, _)| *p == pin) {
                            Some((_, Some(v))) => shown(v),
                            Some((_, None)) => ("немає значення".into(), true, "Джерело ще не мало значення".into()),
                            None if connected => ("—".into(), true, "Під'єднано після останнього виконання".into()),
                            None => ("не під'єднано".into(), true, "Нода використала значення за замовчуванням".into()),
                        },
                        (PinDirection::Output, false) => match trace.outputs.iter().find(|(p, _)| *p == pin) {
                            Some((_, v)) => shown(v),
                            None => ("—".into(), true, "Значення ще не видавалось".into()),
                        },
                    }
                })
            };
            let ty = match &p.pin_type { PinType::Exec => "exec".to_string(), PinType::Data(t) => ed.type_title(t) };
            view! {
                <div class="io-row">
                    <div class="dense-row">
                        <span class="socket connected" class:exec=exec style=format!("--pin-color:{}", ed.types.with_untracked(|types| pin_color(types, &p.pin_type)))></span>
                        <span class="ellipsis" style="flex:1">{p.name}</span>
                        <span class="t-caption">{ty}</span>
                    </div>
                    {move || {
                        let (text, muted, title) = value();
                        (!text.is_empty()).then(|| view! { <div class="io-value" class:muted=muted title=title>{text}</div> })
                    }}
                </div>
            }
        }).collect_view()
    };

    view! {
        <div class="section-title">"Виконання"</div>
        <div class="panel-section">
            <div class="dense-row">
                <span class="t-body-small" style="flex:1">{when}</span>
                <span class="t-caption">{tick}</span>
            </div>
        </div>
        <div class="section-title">"Входи"</div>
        <div class="panel-section">{move || rows(PinDirection::Input)}</div>
        <div class="section-title">"Виходи"</div>
        <div class="panel-section">{move || rows(PinDirection::Output)}</div>
    }
    .into_any()
}

fn shown(value: &DataValue) -> (String, bool, String) {
    let full = display_value(value);
    (truncate(full.clone(), MAX_VALUE_CHARS), false, truncate(full, 5000))
}

#[component]
fn GroupInspector(count: usize) -> impl IntoView {
    let ed = use_editor();
    view! {
        <div class="panel-section">
            <span class="t-row">{format!("Виділено {count} {}", plural(count as u64, "ноду", "ноди", "нод"))}</span>
            <span class="t-body-small">"Тягніть будь-яку з них, щоб перемістити всі. Ctrl+V — вставити копію: ЛКМ ставить, ПКМ скасовує."</span>
        </div>
        <div class="section-title">"Дії"</div>
        <div class="panel-section">
            <ActionButton
                text="Копіювати (Ctrl+C)"
                icon=icons::COPY
                full=true
                flash_on_click=true
                on_click=Callback::new(move |_| if !clipboard::copy_via_command() {
                    ed.log(LogKind::Error, "[ERROR] Браузер не дозволив копіювання — спробуйте Ctrl+C");
                })
            />
            <ActionButton text="Дублювати (Ctrl+D)" icon=icons::DUPLICATE full=true on_click=Callback::new(move |_| clipboard::duplicate(ed)) />
            <ActionButton
                text=format!("Видалити {count} {}", plural(count as u64, "ноду", "ноди", "нод"))
                icon=icons::DELETE
                danger=true
                full=true
                on_click=Callback::new(move |_| clipboard::delete_selection(ed))
            />
        </div>
    }
}

#[component]
fn ConnectionInspector(conn: ConnectionModel, exec: bool) -> impl IntoView {
    let ed = use_editor();
    let pin_name = move |node: NodeId, pin: PinId| {
        ed.node(node)
            .and_then(|n| n.instance.with_untracked(|i| i.definition.pins.iter().find(|p| p.id == pin).map(|p| p.name.clone())))
            .unwrap_or_else(|| format!("#{}", pin.0))
    };
    let from = format!("{} · {}", ed.node_title(conn.src_node), pin_name(conn.src_node, conn.src_pin));
    let to = format!("{} · {}", ed.node_title(conn.dst_node), pin_name(conn.dst_node, conn.dst_pin));
    let color = if exec {
        "var(--text-primary)".to_string()
    } else {
        ed.node(conn.src_node)
            .and_then(|n| n.instance.with_untracked(|i| match crate::geometry::pin_type(&i.definition, conn.src_pin, PinDirection::Output) {
                Some(PinType::Data(t)) => Some(ed.types.with_untracked(|types| type_color(types, t))),
                _ => None,
            }))
            .unwrap_or_else(|| "var(--border-strong)".to_string())
    };
    let disconnect = move |_| {
        ed.send(if exec {
            ClientMessage::DisconnectExec(conn)
        } else {
            ClientMessage::DisconnectData { dst_node: conn.dst_node, dst_pin: conn.dst_pin }
        })
    };
    view! {
        <div class="panel-section">
            <span class="t-row">{if exec { "Exec-з'єднання" } else { "З'єднання даних" }}</span>
        </div>
        <div class="panel-section">
            <div class="dense-row"><span class="socket connected" style=format!("--pin-color:{color}")></span><span class="t-caption">"з"</span></div>
            <span class="t-body-small">{from}</span>
            <div class="dense-row"><span class="socket connected" style=format!("--pin-color:{color}")></span><span class="t-caption">"до"</span></div>
            <span class="t-body-small">{to}</span>
        </div>
        <div class="panel-section">
            <ActionButton text="Від'єднати" icon=icons::DELETE danger=true full=true on_click=Callback::new(disconnect) />
        </div>
    }
}

// --- Консоль ---

#[component]
pub fn ConsolePanel() -> impl IntoView {
    let ed = use_editor();
    view! {
        <div class="console" class:hidden=move || !ed.show_console.get()>
            <div class="console-bar">
                <span class="t-section" style="flex:1">"Консоль"</span>
                <span class="t-caption">{move || format!("{} рядків", ed.logs.with(|l| l.len()))}</span>
                <IconButton icon=icons::BROOM title="Очистити" on_click=Callback::new(move |_| ed.logs.set(Vec::new())) />
                <IconButton icon=icons::CLOSE title="Сховати консоль" on_click=Callback::new(move |_| ed.show_console.set(false)) />
            </div>
            <Terminal lines=ed.logs />
        </div>
    }
}
