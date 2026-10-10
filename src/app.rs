use leptos::prelude::*;
use tetrust_engine::Millis;

use crate::play::stats::{Mode, Snapshot};
use crate::play::PlayApp;
use crate::settings::{Handling, ARR_RANGE, DAS_RANGE, SOFT_DROP_RANGE};

#[component]
pub fn App() -> impl IntoView {
    let mode = RwSignal::new(Mode::Normal);
    let handling = RwSignal::new(Handling::load());
    let stats = RwSignal::new(Snapshot::default());
    // PlayApp은 Rc와 JS 클로저를 들고 있어서 Send가 아니다. 로컬 저장소에 둔다.
    let play = StoredValue::new_local(None::<PlayApp>);

    // 캔버스가 DOM에 붙은 뒤 한 번 실행된다.
    Effect::new(move |_| {
        if play.with_value(Option::is_some) {
            return;
        }
        match PlayApp::mount(Box::new(move |s: &Snapshot| stats.set(s.clone()))) {
            Ok(app) => play.set_value(Some(app)),
            Err(e) => log::error!("failed to start play loop: {e:?}"),
        }
    });
    on_cleanup(move || play.set_value(None));

    let start = move |_| {
        play.with_value(|p| {
            if let Some(p) = p {
                p.start(mode.get_untracked(), handling.get_untracked().config());
            }
        });
    };

    let stat = move |label: &'static str, field: fn(&Snapshot) -> String| {
        view! {
            <div class="flex flex-row justify-between font-mono text-base">
                <span>{label}</span>
                <span>{move || stats.with(field)}</span>
            </div>
        }
    };

    view! {
        <main class="flex justify-center bg-neutral-900 text-neutral-100 min-h-screen">
            <aside class="flex flex-col m-5 gap-4 w-48">
                <div>
                    <p class="font-mono text-2xl text-center">"Hold"</p>
                    <canvas id="hold-canvas" width="120" height="120" class="mx-auto"></canvas>
                </div>
                <div class="font-mono text-center min-h-[4.5rem]">
                    <p>{move || stats.with(|s| s.combo.clone())}</p>
                    <p>{move || stats.with(|s| s.back_to_back.clone())}</p>
                    <p class="text-yellow-300">{move || stats.with(|s| s.message.clone())}</p>
                </div>
                <div class="flex flex-col gap-1">
                    {stat("Time", |s| s.time.clone())}
                    {stat("Lines", |s| s.lines.clone())}
                    {stat("Pieces", |s| s.pieces.clone())}
                    {stat("PPS", |s| s.pps.clone())}
                    {stat("Attack", |s| s.attack.clone())}
                    {stat("Quad", |s| s.quads.clone())}
                    {stat("PC", |s| s.perfect_clears.clone())}
                </div>
                <button
                    class="bg-blue-600 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded-full"
                    on:click=start
                >
                    "Start (R)"
                </button>
                <fieldset class="font-mono text-sm">
                    <label class="block">
                        <input
                            type="radio"
                            name="mode"
                            prop:checked=move || mode.get() == Mode::Normal
                            on:change=move |_| mode.set(Mode::Normal)
                        />
                        " Normal"
                    </label>
                    <label class="block">
                        <input
                            type="radio"
                            name="mode"
                            prop:checked=move || mode.get() == Mode::Sprint
                            on:change=move |_| mode.set(Mode::Sprint)
                        />
                        " Sprint (40 Lines)"
                    </label>
                </fieldset>
            </aside>

            <div class="mt-5 mr-2">
                <canvas id="garbage-gauge-canvas" width="30" height="660"></canvas>
            </div>
            <section class="my-5">
                <canvas id="game-canvas" width="300" height="660"></canvas>
            </section>

            <aside class="m-5 flex flex-col gap-4 w-48">
                <div>
                    <p class="font-mono text-2xl text-center">"Next"</p>
                    <canvas id="next-canvas" width="120" height="520" class="mx-auto"></canvas>
                </div>
                <HandlingPanel handling=handling/>
                <p class="font-mono text-xs leading-5 text-neutral-400">
                    "← → 이동 · ↓ 소프트 드롭 · Space 하드 드롭"<br/>
                    "↑/X 시계 · Z 반시계 · A 180 · Shift/C 홀드 · R 재시작"
                </p>
            </aside>
        </main>
    }
}

/// DAS·ARR·소프트 드롭 설정. 바꾼 값은 저장되고 다음 판부터 적용된다.
#[component]
fn HandlingPanel(handling: RwSignal<Handling>) -> impl IntoView {
    let field = move |label: &'static str,
                      range: (Millis, Millis),
                      get: fn(&Handling) -> Millis,
                      set: fn(&mut Handling, Millis)| {
        view! {
            <label class="flex flex-row justify-between items-center gap-2">
                <span>{label}</span>
                <input
                    type="number"
                    class="w-20 bg-neutral-800 px-1 text-right"
                    min=range.0
                    max=range.1
                    prop:value=move || handling.with(get).to_string()
                    on:change=move |ev| {
                        let Ok(value) = event_target_value(&ev).trim().parse::<Millis>() else {
                            return;
                        };
                        handling.update(|h| {
                            set(h, value);
                            *h = h.clamped();
                        });
                        handling.get_untracked().save();
                    }
                />
            </label>
        }
    };

    view! {
        <fieldset class="font-mono text-sm flex flex-col gap-1">
            <legend class="mb-1">"Handling (ms)"</legend>
            {field("DAS", DAS_RANGE, |h| h.das, |h, v| h.das = v)}
            {field("ARR", ARR_RANGE, |h| h.arr, |h, v| h.arr = v)}
            {field("Soft drop", SOFT_DROP_RANGE, |h| h.soft_drop, |h, v| h.soft_drop = v)}
            <p class="text-xs text-neutral-400">"0은 즉시. 다음 판부터 적용"</p>
        </fieldset>
    }
}
