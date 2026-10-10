//! 브라우저에서 엔진을 돌리는 계층.
//!
//! - 시뮬레이션은 `requestAnimationFrame` 루프 하나가 게임 시계를 진행시킨다.
//!   setTimeout·setInterval 타이머는 쓰지 않는다.
//! - 키 입력은 `event.timeStamp`로 눌린 시각을 잡아 엔진 입력 로그에 기록한다.
//! - 이벤트 리스너와 rAF 루프는 [`PlayApp`]이 소유하고, 드롭될 때 모두 해제한다.

pub mod input;
pub mod render;
pub mod stats;

use std::cell::RefCell;
use std::rc::Rc;

use tetrust_engine::{Config, Event, EventKind, Game};
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, KeyboardEvent, Window};

use render::Renderer;
use stats::{Mode, Snapshot, Stats};

/// 한 판의 상태.
struct Session {
    game: Game,
    /// 매치 시작 시각 (`performance.now()` 기준 ms).
    start: f64,
    seq: u64,
    /// 리플레이·서버 검증용 입력 로그.
    log: Vec<Event>,
    mode: Mode,
    stats: Stats,
}

impl Session {
    fn new(mode: Mode, config: Config, start: f64) -> Self {
        let seed = (js_sys::Math::random() * (1u64 << 53) as f64) as u64;
        Self {
            game: Game::new(config, seed),
            start,
            seq: 0,
            log: Vec::new(),
            mode,
            stats: Stats::default(),
        }
    }

    fn collect_outcomes(&mut self) {
        for outcome in self.game.drain_outcomes() {
            self.stats.apply(&outcome, self.mode);
        }
    }

    fn input(&mut self, timestamp: f64, kind: EventKind) {
        if self.stats.is_finished() {
            return;
        }
        let time = input::game_time(timestamp, self.start, self.game.clock());
        let event = Event::new(time, self.seq, kind);
        self.seq += 1;
        match self.game.handle(event) {
            Ok(()) => self.log.push(event),
            Err(e) => log::warn!("dropped input {event:?}: {e:?}"),
        }
        self.collect_outcomes();
    }

    fn frame(&mut self, timestamp: f64) {
        if self.stats.is_finished() {
            return;
        }
        let time = input::game_time(timestamp, self.start, self.game.clock());
        self.game.advance_to(time);
        self.collect_outcomes();
    }

    /// 화면에 보여 줄 경과 시간. 스프린트가 끝났으면 완료 시각에서 멈춘다.
    fn display_time(&self) -> u64 {
        self.stats.finished_at.unwrap_or(self.game.clock())
    }
}

/// 통계가 바뀔 때마다 UI에 알려 주는 콜백.
pub type OnStats = Box<dyn Fn(&Snapshot)>;

struct State {
    session: Option<Session>,
    renderer: Renderer,
    on_stats: OnStats,
    last_snapshot: Snapshot,
    /// 다음 판의 모드와 설정. R 키 재시작도 이 값을 쓴다.
    mode: Mode,
    config: Config,
}

impl State {
    fn start(&mut self, now: f64) {
        self.session = Some(Session::new(self.mode, self.config.clone(), now));
    }

    fn draw(&mut self) {
        let Some(session) = &self.session else {
            self.renderer.draw_idle();
            return;
        };
        self.renderer.draw(&session.game);
        let snapshot = Snapshot::new(&session.stats, session.display_time());
        if snapshot != self.last_snapshot {
            (self.on_stats)(&snapshot);
            self.last_snapshot = snapshot;
        }
    }
}

type FrameCallback = Closure<dyn FnMut(f64)>;
type KeyCallback = Closure<dyn FnMut(KeyboardEvent)>;

/// 화면 하나에 붙는 플레이 루프. 드롭하면 리스너와 rAF가 해제된다.
pub struct PlayApp {
    window: Window,
    document: Document,
    state: Rc<RefCell<State>>,
    keydown: KeyCallback,
    keyup: KeyCallback,
    frame: Rc<RefCell<Option<FrameCallback>>>,
    frame_handle: Rc<std::cell::Cell<i32>>,
}

/// 설정 입력칸에 타이핑 중이면 게임 키로 쓰지 않는다.
fn typing_in_field(event: &KeyboardEvent) -> bool {
    event.target().is_some_and(|t| {
        t.dyn_ref::<web_sys::HtmlInputElement>()
            .is_some_and(|i| i.type_() == "number")
    })
}

fn now(window: &Window) -> f64 {
    window.performance().map_or(0.0, |p| p.now())
}

impl PlayApp {
    /// `on_stats`는 사이드바 통계가 바뀐 프레임에만 불린다.
    pub fn mount(on_stats: OnStats) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let state = Rc::new(RefCell::new(State {
            session: None,
            renderer: Renderer::new(&document)?,
            on_stats,
            last_snapshot: Snapshot::default(),
            mode: Mode::default(),
            config: Config::default(),
        }));

        let keydown = {
            let state = Rc::clone(&state);
            let window = window.clone();
            Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
                if typing_in_field(&event) {
                    return;
                }
                let code = event.code();
                if code == "KeyR" && !event.repeat() {
                    state.borrow_mut().start(now(&window));
                    return;
                }
                let Some(action) = input::action_for(&code) else {
                    return;
                };
                // 스크롤, 버튼 활성화 같은 브라우저 기본 동작을 막는다.
                event.prevent_default();
                if event.repeat() {
                    return;
                }
                if let Some(session) = state.borrow_mut().session.as_mut() {
                    session.input(event.time_stamp(), EventKind::Press(action));
                }
            })
        };
        let keyup = {
            let state = Rc::clone(&state);
            Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
                if typing_in_field(&event) {
                    return;
                }
                let Some(action) = input::action_for(&event.code()) else {
                    return;
                };
                event.prevent_default();
                if let Some(session) = state.borrow_mut().session.as_mut() {
                    session.input(event.time_stamp(), EventKind::Release(action));
                }
            })
        };
        document.add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref())?;
        document.add_event_listener_with_callback("keyup", keyup.as_ref().unchecked_ref())?;

        let frame: Rc<RefCell<Option<FrameCallback>>> = Rc::new(RefCell::new(None));
        let frame_handle = Rc::new(std::cell::Cell::new(0));
        {
            let state = Rc::clone(&state);
            let window_for_loop = window.clone();
            let next = Rc::clone(&frame);
            let handle = Rc::clone(&frame_handle);
            *frame.borrow_mut() = Some(Closure::new(move |timestamp: f64| {
                {
                    let mut state = state.borrow_mut();
                    if let Some(session) = state.session.as_mut() {
                        session.frame(timestamp);
                    }
                    state.draw();
                }
                if let Some(callback) = next.borrow().as_ref() {
                    if let Ok(id) =
                        window_for_loop.request_animation_frame(callback.as_ref().unchecked_ref())
                    {
                        handle.set(id);
                    }
                }
            }));
        }
        if let Some(callback) = frame.borrow().as_ref() {
            frame_handle.set(window.request_animation_frame(callback.as_ref().unchecked_ref())?);
        }

        Ok(Self {
            window,
            document,
            state,
            keydown,
            keyup,
            frame,
            frame_handle,
        })
    }

    /// 새 판을 시작한다.
    pub fn start(&self, mode: Mode, config: Config) {
        let mut state = self.state.borrow_mut();
        state.mode = mode;
        state.config = config;
        state.start(now(&self.window));
    }

    /// 지금 판의 입력 로그. 리플레이와 대전 서버 검증(4단계)에서 쓴다.
    #[allow(dead_code)]
    pub fn input_log(&self) -> Vec<Event> {
        self.state
            .borrow()
            .session
            .as_ref()
            .map(|s| s.log.clone())
            .unwrap_or_default()
    }
}

impl Drop for PlayApp {
    fn drop(&mut self) {
        let _ = self.window.cancel_animation_frame(self.frame_handle.get());
        // 클로저가 자기 자신을 붙잡고 있는 순환 참조를 끊는다.
        self.frame.borrow_mut().take();
        let _ = self
            .document
            .remove_event_listener_with_callback("keydown", self.keydown.as_ref().unchecked_ref());
        let _ = self
            .document
            .remove_event_listener_with_callback("keyup", self.keyup.as_ref().unchecked_ref());
    }
}
