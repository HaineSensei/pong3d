use glam::Vec3;

// Room dimensions. The room is an open-ended tube: solid walls on the
// four sides (x and y bounds), but no wall at z = 0 or z = DEPTH -- those
// are the "goal planes" where a missed ball scores a point.
pub const HALF_W: f32 = 6.0;
pub const HALF_H: f32 = 4.0;
pub const DEPTH: f32 = 24.0;

pub const PADDLE_HALF: f32 = 1.0;
pub const PADDLE_Z_PLAYER: f32 = 1.4;
pub const PADDLE_Z_AI: f32 = DEPTH - 1.4;
pub const PADDLE_SPEED: f32 = 9.0;
pub const AI_SPEED: f32 = 6.2;

pub const BALL_RADIUS: f32 = 0.35;
pub const BALL_BASE_SPEED: f32 = 9.0;
pub const BALL_MAX_SPEED: f32 = 22.0;

pub const WIN_SCORE: u32 = 11;
pub const WIN_MARGIN: u32 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Winner {
    Player,
    Ai,
}

pub struct Game {
    pub player_pos: glam::Vec2, // x,y of player paddle center
    pub ai_pos: glam::Vec2,
    pub ball_pos: Vec3,
    pub ball_vel: Vec3,
    pub player_score: u32,
    pub ai_score: u32,
    pub winner: Option<Winner>,

    pub move_left: bool,
    pub move_right: bool,
    pub move_up: bool,
    pub move_down: bool,

    rng_state: u32,
}

impl Game {
    pub fn new() -> Self {
        let mut g = Self {
            player_pos: glam::Vec2::ZERO,
            ai_pos: glam::Vec2::ZERO,
            ball_pos: Vec3::new(0.0, 0.0, DEPTH / 2.0),
            ball_vel: Vec3::ZERO,
            player_score: 0,
            ai_score: 0,
            winner: None,
            move_left: false,
            move_right: false,
            move_up: false,
            move_down: false,
            rng_state: 0x9E3779B9,
        };
        g.serve(true);
        g
    }

    fn next_rand(&mut self) -> f32 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = x;
        (x % 10_000) as f32 / 10_000.0
    }

    fn serve(&mut self, toward_ai: bool) {
        self.ball_pos = Vec3::new(0.0, 0.0, DEPTH / 2.0);
        let rx = (self.next_rand() - 0.5) * 0.6;
        let ry = (self.next_rand() - 0.5) * 0.6;
        let dir_z = if toward_ai { 1.0 } else { -1.0 };
        let v = Vec3::new(rx, ry, dir_z).normalize_or_zero();
        self.ball_vel = v * BALL_BASE_SPEED;
    }

    pub fn restart(&mut self) {
        self.player_score = 0;
        self.ai_score = 0;
        self.winner = None;
        self.player_pos = glam::Vec2::ZERO;
        self.ai_pos = glam::Vec2::ZERO;
        self.serve(true);
    }

    fn paddle_bounds() -> (f32, f32) {
        (HALF_W - PADDLE_HALF, HALF_H - PADDLE_HALF)
    }

    pub fn update(&mut self, dt: f32) {
        if self.winner.is_some() {
            return;
        }

        // --- player paddle movement ---
        let (max_x, max_y) = Self::paddle_bounds();
        let mut dx = 0.0f32;
        let mut dy = 0.0f32;
        if self.move_left {
            dx -= 1.0;
        }
        if self.move_right {
            dx += 1.0;
        }
        if self.move_up {
            dy += 1.0;
        }
        if self.move_down {
            dy -= 1.0;
        }
        let mv = glam::Vec2::new(dx, dy).normalize_or_zero() * PADDLE_SPEED * dt;
        self.player_pos = (self.player_pos + mv).clamp(
            glam::Vec2::new(-max_x, -max_y),
            glam::Vec2::new(max_x, max_y),
        );

        // --- simple AI paddle ---
        let target = if self.ball_vel.z > 0.0 {
            glam::Vec2::new(self.ball_pos.x, self.ball_pos.y)
        } else {
            glam::Vec2::ZERO
        };
        let to_target = target - self.ai_pos;
        let step = AI_SPEED * dt;
        if to_target.length() > step {
            self.ai_pos += to_target.normalize() * step;
        } else {
            self.ai_pos = target;
        }
        self.ai_pos = self.ai_pos.clamp(
            glam::Vec2::new(-max_x, -max_y),
            glam::Vec2::new(max_x, max_y),
        );

        // --- ball integration with swept collision against paddle planes ---
        let prev = self.ball_pos;
        let next = prev + self.ball_vel * dt;

        // wall bounces (x/y), simple reflect-and-clamp
        let mut next = next;
        let mut vel = self.ball_vel;
        if next.x - BALL_RADIUS < -HALF_W {
            next.x = -HALF_W + BALL_RADIUS;
            vel.x = vel.x.abs();
        } else if next.x + BALL_RADIUS > HALF_W {
            next.x = HALF_W - BALL_RADIUS;
            vel.x = -vel.x.abs();
        }
        if next.y - BALL_RADIUS < -HALF_H {
            next.y = -HALF_H + BALL_RADIUS;
            vel.y = vel.y.abs();
        } else if next.y + BALL_RADIUS > HALF_H {
            next.y = HALF_H - BALL_RADIUS;
            vel.y = -vel.y.abs();
        }

        // swept check against player paddle plane (z = PADDLE_Z_PLAYER), only
        // relevant while the ball travels toward the player (-z).
        if vel.z < 0.0 && prev.z >= PADDLE_Z_PLAYER && next.z < PADDLE_Z_PLAYER {
            let t = (PADDLE_Z_PLAYER - prev.z) / (next.z - prev.z);
            let hit = prev.lerp(next, t.clamp(0.0, 1.0));
            if (hit.x - self.player_pos.x).abs() <= PADDLE_HALF + BALL_RADIUS
                && (hit.y - self.player_pos.y).abs() <= PADDLE_HALF + BALL_RADIUS
            {
                let off_x = (hit.x - self.player_pos.x) / PADDLE_HALF;
                let off_y = (hit.y - self.player_pos.y) / PADDLE_HALF;
                let speed = (vel.length() * 1.06).min(BALL_MAX_SPEED);
                let dir = Vec3::new(off_x * 0.9, off_y * 0.9, 1.0).normalize_or_zero();
                vel = dir * speed;
                next = hit + Vec3::new(0.0, 0.0, 0.02);
            }
        }

        // swept check against AI paddle plane (z = PADDLE_Z_AI).
        if vel.z > 0.0 && prev.z <= PADDLE_Z_AI && next.z > PADDLE_Z_AI {
            let t = (PADDLE_Z_AI - prev.z) / (next.z - prev.z);
            let hit = prev.lerp(next, t.clamp(0.0, 1.0));
            if (hit.x - self.ai_pos.x).abs() <= PADDLE_HALF + BALL_RADIUS
                && (hit.y - self.ai_pos.y).abs() <= PADDLE_HALF + BALL_RADIUS
            {
                let off_x = (hit.x - self.ai_pos.x) / PADDLE_HALF;
                let off_y = (hit.y - self.ai_pos.y) / PADDLE_HALF;
                let speed = (vel.length() * 1.06).min(BALL_MAX_SPEED);
                let dir = Vec3::new(off_x * 0.9, off_y * 0.9, -1.0).normalize_or_zero();
                vel = dir * speed;
                next = hit - Vec3::new(0.0, 0.0, 0.02);
            }
        }

        self.ball_pos = next;
        self.ball_vel = vel;

        // --- scoring: ball passed through a goal plane without being hit ---
        if self.ball_pos.z < -0.5 {
            self.ai_score += 1;
            self.after_point();
        } else if self.ball_pos.z > DEPTH + 0.5 {
            self.player_score += 1;
            self.after_point();
        }
    }

    fn after_point(&mut self) {
        let leader_serves_next = self.player_score > self.ai_score;
        if (self.player_score >= WIN_SCORE || self.ai_score >= WIN_SCORE)
            && self.player_score.abs_diff(self.ai_score) >= WIN_MARGIN
        {
            self.winner = Some(if self.player_score > self.ai_score {
                Winner::Player
            } else {
                Winner::Ai
            });
            return;
        }
        self.serve(leader_serves_next);
    }
}
