use glam::{Mat4, Vec2, Vec3};
use sdl3::{event, keyboard::Keycode, mouse::MouseButton};
use std::time::Instant;
use voxel_rendering::v2::Renderer;

pub mod schedule;

struct CameraController {
    position: Vec3,
    yaw: f32,   // radians, rotation around world Y (left/right look)
    pitch: f32, // radians, rotation around local X (up/down look)
    speed: f32, // units per second
    sensitivity: f32, // radians per pixel of mouse movement
}

impl CameraController {
    fn new(position: Vec3) -> Self {
        Self {
            position,
            yaw: -std::f32::consts::FRAC_PI_2, // start facing -Z (toward origin from +Z-ish start)
            pitch: 0.0,
            speed: 5.0,
            sensitivity: 0.0025,
        }
    }

    fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    fn handle_mouse_motion(&mut self, xrel: f32, yrel: f32) {
        self.yaw += xrel * self.sensitivity;
        self.pitch -= yrel * self.sensitivity; // inverted: mouse up -> look up

        // Clamp pitch to avoid gimbal flip at the poles
        let limit = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = self.pitch.clamp(-limit, limit);
    }

    fn handle_keys(&mut self, keys_held: &KeysHeld, dt: f32) {
        let forward = self.forward();
        let right = self.right();
        let mut movement = Vec3::ZERO;

        if keys_held.forward { movement += forward; }
        if keys_held.backward { movement -= forward; }
        if keys_held.right { movement += right; }
        if keys_held.left { movement -= right; }
        if keys_held.up { movement += Vec3::Y; }
        if keys_held.down { movement -= Vec3::Y; }

        if movement.length_squared() > 0.0 {
            self.position += movement.normalize() * self.speed * dt;
        }
    }

    fn transform(&self) -> Mat4 {
        let forward = self.forward();
        let right = self.right();
        let true_up = right.cross(forward); // re-derive to guarantee orthogonality

        // Camera space looks down its local -Z axis by convention, so the
        // local +Z basis vector (column 2) maps to "backward" in world space.
        Mat4::from_cols(
            right.extend(0.0),
            true_up.extend(0.0),
            (-forward).extend(0.0),
            self.position.extend(1.0),
        )
    }
}

#[derive(Default)]
struct KeysHeld {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
}

pub struct App {
    sdl: sdl3::Sdl,
    video_subsystem: sdl3::VideoSubsystem,
    window: sdl3::video::Window,
    camera: voxel_rendering::v2::Camera,
    controller: CameraController,
    keys_held: KeysHeld,
    mouse_captured: bool,
    scenario: voxel_rendering::v2::ScenarioHandle,
    pub renderer: Renderer,
}

impl App {
    pub fn new() -> Self {
        let sdl = sdl3::init().unwrap();
        let video_subsystem = sdl.video().unwrap();

        let window = video_subsystem
            .window("Voxel Engine", 800, 600)
            .resizable()
            .position_centered()
            .vulkan()
            .build()
            .unwrap();

        let mut r = Renderer::init(window.clone());
        let scenario = r.create_scenario();

        let mesh = r.load_obj(scenario, &format!("assets/{}.obj", "teapot"));
        r.create_instance(scenario, Mat4::IDENTITY, mesh);

        // let mesh2 = r.load_obj(scenario, &format!("assets/{}.obj", "stanford-bunny"));
        // r.create_instance(scenario, Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)), mesh2);

        let camera = voxel_rendering::v2::Camera::new(45.0, 0.1, 100.0);
        let controller = CameraController::new(Vec3::new(0.0, 0.0, -10.0));

        App {
            sdl,
            video_subsystem,
            window,
            camera,
            controller,
            keys_held: KeysHeld::default(),
            mouse_captured: false,
            scenario,
            renderer: r,
        }
    }

    pub fn run(&mut self) {
        let mut event_pump = self.sdl.event_pump().unwrap();
        let mut last_frame = Instant::now();

        'running: loop {
            let now = Instant::now();
            let dt = (now - last_frame).as_secs_f32();
            last_frame = now;

            for event in event_pump.poll_iter() {
                match event {
                    event::Event::Quit { .. }
                    | event::Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => break 'running,

                    event::Event::Window {
                        win_event: event::WindowEvent::Resized(width, height),
                        ..
                    } => {
                        self.renderer.resize(width as u32, height as u32);
                    }
                    event::Event::Window {
                        win_event: event::WindowEvent::Minimized,
                        ..
                    } => {
                        self.renderer.resize(0, 0);
                    }

                    event::Event::MouseMotion { xrel, yrel, .. } => {
                        if self.mouse_captured {
                            self.controller.handle_mouse_motion(xrel as f32, yrel as f32);
                        }
                    }

                    // Optional: click to re-capture if you ever release the
                    // mouse (e.g. via a debug-UI toggle key you add later).
                    event::Event::MouseButtonDown { mouse_btn: MouseButton::Left, .. } => {
                        if !self.mouse_captured {
                            self.sdl.mouse().set_relative_mouse_mode(&self.window, true);
                            self.mouse_captured = true;
                        }
                    }

                    event::Event::KeyDown { keycode: Some(kc), .. } => match kc {
                        Keycode::W => self.keys_held.forward = true,
                        Keycode::S => self.keys_held.backward = true,
                        Keycode::A => self.keys_held.left = true,
                        Keycode::D => self.keys_held.right = true,
                        Keycode::Space => self.keys_held.up = true,
                        Keycode::LShift => self.keys_held.down = true,
                        _ => {}
                    },
                    event::Event::KeyUp { keycode: Some(kc), .. } => match kc {
                        Keycode::W => self.keys_held.forward = false,
                        Keycode::S => self.keys_held.backward = false,
                        Keycode::A => self.keys_held.left = false,
                        Keycode::D => self.keys_held.right = false,
                        Keycode::Space => self.keys_held.up = false,
                        Keycode::LShift => self.keys_held.down = false,
                        _ => {}
                    },

                    _ => {}
                }
            }

            self.controller.handle_keys(&self.keys_held, dt);
            self.camera.set_transform(self.controller.transform());

            self.renderer.update_camera(
                self.scenario,
                &self.camera,
                self.window.size().0 as f32 / self.window.size().1 as f32,
            );
            self.renderer.draw();
        }
    }
}