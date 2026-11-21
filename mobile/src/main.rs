//! Desktop and web entry point. Android loads the library instead (see `lib.rs`).

fn window_conf() -> macroquad::window::Conf {
    kitty_mobile::window_conf()
}

#[macroquad::main(window_conf)]
async fn main() {
    kitty_mobile::run().await;
}
