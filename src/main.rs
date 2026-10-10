pub mod components;
pub mod play;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<components::main::MainComponent>::new().render();
}
