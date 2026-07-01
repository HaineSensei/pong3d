mod game;
mod geometry;
mod gfx;
mod hud;

use std::sync::Arc;
use std::time::Instant;

use game::{Game, Winner};
use gfx::Graphics;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

struct App {
    window: Option<Arc<Window>>,
    gfx: Option<Graphics>,
    game: Game,
    last_frame: Instant,
    start: Instant,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            gfx: None,
            game: Game::new(),
            last_frame: Instant::now(),
            start: Instant::now(),
        }
    }

    fn set_key(&mut self, code: KeyCode, pressed: bool) {
        match code {
            KeyCode::KeyA | KeyCode::ArrowLeft => self.game.move_left = pressed,
            KeyCode::KeyD | KeyCode::ArrowRight => self.game.move_right = pressed,
            KeyCode::KeyW | KeyCode::ArrowUp => self.game.move_up = pressed,
            KeyCode::KeyS | KeyCode::ArrowDown => self.game.move_down = pressed,
            KeyCode::KeyR => {
                if pressed {
                    self.game.restart();
                }
            }
            _ => {}
        }
    }

    fn update_title(&self) {
        let Some(window) = &self.window else { return };
        let title = match self.game.winner {
            None => format!(
                "Pong 3D  —  You {} : {} CPU  (first to {}, win by {})",
                self.game.player_score,
                self.game.ai_score,
                game::WIN_SCORE,
                game::WIN_MARGIN
            ),
            Some(Winner::Player) => format!(
                "Pong 3D  —  YOU WIN {} : {}  —  press R to play again",
                self.game.player_score, self.game.ai_score
            ),
            Some(Winner::Ai) => format!(
                "Pong 3D  —  CPU WINS {} : {}  —  press R to play again",
                self.game.ai_score, self.game.player_score
            ),
        };
        window.set_title(&title);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = WindowAttributes::default()
            .with_title("Pong 3D")
            .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 720.0));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        let gfx = pollster::block_on(Graphics::new(window.clone()));

        self.window = Some(window);
        self.gfx = Some(gfx);
        self.last_frame = Instant::now();
        self.update_title();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                if code == KeyCode::Escape && state == ElementState::Pressed {
                    event_loop.exit();
                    return;
                }
                self.set_key(code, state == ElementState::Pressed);
            }
            WindowEvent::Resized(size) => {
                if let Some(gfx) = self.gfx.as_mut() {
                    gfx.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last_frame).as_secs_f32().min(0.05);
                self.last_frame = now;

                let had_winner = self.game.winner.is_some();
                self.game.update(dt);
                if self.game.winner.is_some() != had_winner {
                    self.update_title();
                }

                let flash = gfx::winner_flash(
                    self.game.winner,
                    (self.start.elapsed().as_secs_f32() * 3.0).sin() * 0.5 + 0.5,
                );

                if let Some(gfx) = self.gfx.as_mut() {
                    gfx.render(&self.game, flash);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("event loop error");
}
