//! Car physics handling.
use std::f32::consts::PI;

use bevy::{
    input::gamepad::{GamepadConnection, GamepadConnectionEvent},
    prelude::*,
    window::PrimaryWindow,
};
use lazy_static::lazy_static;
pub struct CarPlugin;

/// Plugin for all our systems.
impl Plugin for CarPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup_car)
            .add_system(update_velocity)
            .add_system(gamepad_connections)
            .add_system(control_throttle);
    }
}

/// The model of the car, defining constants.
#[derive(Component, Clone)]
struct Model {
    /// Drag coeficient, based on car aerodynamics.
    drag: f32,
    /// Rolling resistance, roughly 30x drag.
    rolling_resistance: f32,
    /// Mass in kilograms
    mass: f32,
    /// Ratio of differential.
    differential_ratio: f32,
    /// Gear ratios. The length of the Vec is the amount of gears (not including reverse).
    gear_ratios: Vec<f32>,
    /// Wheel radius.
    wheel_radius: f32,
    /// Efficiency of the transmission.
    transmission_efficiency: f32,
    /// Engine torque per RPM. Torque for arbitrary RPMs will be calculated with linear interpolation.
    engine_torque: Vec<(f32, f32)>,
    /// Constant for brake force, I have no idea how to find out what it should be.
    brakes: f32,
}

impl Model {
    /// Minimum engine RPM.
    fn min_rpm(&self) -> f32 {
        self.engine_torque[0].0
    }

    /*fn max_rpm(&self) -> f32 {
        self.engine_torque[self.engine_torque.len()-1].0
    }*/

    /// Best engine RPM for torque.
    fn best_rpm(&self) -> f32 {
        self.engine_torque
            .iter()
            .max_by(|r0, r1| r0.1.partial_cmp(&r1.1).unwrap())
            .unwrap()
            .0
    }
}

lazy_static! {
    /// Specs for a Corvette C5, apparently.
    static ref CORVETTE: Model = Model {
        drag: 0.4257,
        rolling_resistance: 12.8,
        mass: 1500.0,
        differential_ratio: 3.42,
        gear_ratios: vec![2.66, 1.78, 1.30, 1.0, 0.74, 0.50],
        wheel_radius: 0.33,
        transmission_efficiency: 0.7,
        engine_torque: vec![
            (1000.0, 450.0),
            (1500.0, 480.0),
            (3000.0, 490.0),
            (5000.0, 500.0),
            (5800.0, 450.0)
        ],
        brakes: 12000.0,
    };
}

/// An actual moving car
#[derive(Component, Default)]
pub struct Car {
    /// Direction (constant for now, we don't have a steering wheel).
    direction: Vec2,
    /// Velocity vector.
    velocity: Vec2,
    /// Speed in M/S.
    pub speed: f32,
    /// Throttle 0.0 no throttle, 1.0 full power, -1.0 full brakes.
    throttle: f32,
    /// Current gear.
    pub gear: usize,
    /// Current RPM.
    pub rpm: f32,
    /// Keep track of which speed we geared up, from 2nd gear upward, we just gear down when reaching this speed when braking.
    speeds: Vec<f32>,
}

/// Mark the car the player operates.
#[derive(Component)]
pub struct Player {}

/// Setup the starting state of the car, not moving, in first gear.
fn setup_car(mut commands: Commands) {
    commands.spawn((
        Player {},
        Car {
            direction: Vec2::X,
            gear: 1,
            ..Car::default()
        },
        CORVETTE.clone(),
    ));
}

/// Lookup the torque for the given engine rpm.
fn lookup_torque(model: &Model, engine_rpm: f32) -> f32 {
    for ix in 0..model.engine_torque.len() {
        let (rpm, torque) = model.engine_torque[ix];
        if rpm == engine_rpm {
            return torque;
        } else if rpm > engine_rpm && ix > 0 {
            let (rpm0, torque0) = model.engine_torque[ix - 1];
            return (torque0 * (rpm - engine_rpm) + torque * (engine_rpm - rpm0)) / (rpm - rpm0);
        }
    }
    0.0
}

/// Angular speed of the wheels.
fn wheel_speed(car: &Car, model: &Model) -> f32 {
    car.speed / model.wheel_radius
}

/// Engine rpm based on the current wheel speed of the given car.
fn engine_rpm(car: &Car, model: &Model, wheel_speed: f32) -> f32 {
    wheel_speed * model.gear_ratios[car.gear - 1] * model.differential_ratio * 60.0 / (2.0 * PI)
}

/// Update the velocity and speed of the car.
/// <https://asawicki.info/Mirror/Car%20Physics%20for%20Games/Car%20Physics%20for%20Games.html>
fn update_velocity(time: Res<Time>, mut cars: Query<(&mut Car, &Model), With<Player>>) {
    for (mut car, model) in &mut cars {
        // Update rpm based on wheel speed.
        car.rpm = engine_rpm(&car, model, wheel_speed(&car, model));
        //println!("rpm: {:.2}",car.rpm);
        let min = model.min_rpm();
        let best = model.best_rpm();
        // Sanity.
        if car.rpm < min {
            car.rpm = min;
        // Switch gear,
        } else if car.rpm >= best && car.gear < model.gear_ratios.len() {
            //println!("{:.2}",car.rpm);
            car.rpm = min;
            car.gear += 1;
            let g = car.gear;
            // Keep track of the speeds we switched gears on.
            if g > 1 {
                let s = car.speed;
                if car.speeds.len() < g - 1 {
                    car.speeds.push(s);
                } else {
                    car.speeds[g - 2] = s;
                }
            }
        }
        // Forced applied by the player.
        let control = if car.throttle > 0.0 {
            // Acceleration.
            //println!("rpm: {rpm} gear: {}", car.gear);
            let max_torque = lookup_torque(model, car.rpm);
            let engine_torque = max_torque * car.throttle;
            //let traction = car.direction * car.engine_force;
            car.direction
                * engine_torque
                * model.gear_ratios[car.gear - 1]
                * model.differential_ratio
                * model.transmission_efficiency
                / model.wheel_radius
            // Brakes.
        } else if car.throttle < 0.0 {
            car.direction * car.throttle * model.brakes
        } else {
            // Nothing.
            Vec2::ZERO
        };
        //println!("{control}");
        // Drag.
        let drag = -model.drag * car.velocity * car.speed;
        // Rolling resistance.
        let rolling_resistance = -model.rolling_resistance * car.velocity;
        //println!("traction: {traction}, drag: {drag}, rr: {rolling_resistance}");
        // Full longitudinal force.
        let longitudinal = control + drag + rolling_resistance;
        // Acceleration.
        let acceleration = longitudinal / model.mass;
        // Current velocity and speed.
        car.velocity += acceleration * time.delta_seconds();
        car.speed = car.velocity.length();
        //println!("{}",car.velocity.angle_between(car.direction) > 90.0 * PI / 180.0);
        //println!("{} {}", car.velocity,car.velocity.angle_between(car.direction));

        if car.throttle < 0.0 {
            // Ensure we stop braking when reaching zero.
            if car.speed == 0.0
                || (!car.velocity.is_nan()
                    && car.velocity.angle_between(car.direction).abs() > PI / 2.0)
            {
                car.speed = 0.0;
                car.velocity = Vec2::ZERO;
                if car.throttle < 0.0 {
                    car.throttle = 0.0
                }
                car.gear = 1;
                // Downgear.
            } else if car.gear > 1
                && car.speeds.len() > car.gear - 2
                && car.speed < car.speeds[car.gear - 2]
            {
                car.gear -= 1;
            }
        }
        //println!("speed: {:.2}", car.speed);∂Ò
    }
}

/// Throttle control.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Throttle {
    Accelerate,
    Brake,
    Roll,
}

/// Control the throttle based on input.
fn control_throttle(
    window: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    mouse: Option<Res<Input<MouseButton>>>,
    keyboard: Option<Res<Input<KeyCode>>>,
    gamepad: Option<Res<Input<GamepadButton>>>,
    my_gamepad: Option<Res<MyGamepad>>,
    mut cars: Query<&mut Car, With<Player>>,
) {
    if let Ok(mut car) = cars.get_single_mut() {
        let mut throttle = Throttle::Roll;
        // Click on pedals.
        if let Some(input) = &mouse {
            if let Ok(window) = window.get_single() {
                if input.pressed(MouseButton::Left) {
                    if let Some(pos) = window.cursor_position() {
                        //println!("mouse: {pos}");
                        if pos.x >= 515.0 && pos.x <= 665.0 && pos.y >= 40.0 && pos.y <= 128.0 {
                            throttle = Throttle::Brake;
                        } else if pos.x >= 691.0
                            && pos.x <= 758.0
                            && pos.y >= 19.0
                            && pos.y <= 152.0
                        {
                            throttle = Throttle::Accelerate;
                        }
                    }
                }
            }
        }
        // If no pedal clicked, check keyboard up/down.
        if throttle == Throttle::Roll {
            if let Some(input) = &keyboard {
                if input.pressed(KeyCode::Up) && car.throttle < 1.0 {
                    throttle = Throttle::Accelerate;
                } else if input.pressed(KeyCode::Down) && car.throttle > -1.0 {
                    throttle = Throttle::Brake;
                }
            }
        }
        // If no mouse or keyboard, check gamepad.
        if throttle == Throttle::Roll {
            if let Some(input) = &gamepad {
                if let Some(gp) = &my_gamepad {
                    // a gamepad is connected, we have the id
                    let gamepad = gp.0;

                    let accelerate_button = GamepadButton {
                        gamepad,
                        button_type: GamepadButtonType::RightTrigger2,
                    };
                    let brake_button = GamepadButton {
                        gamepad,
                        button_type: GamepadButtonType::LeftTrigger2,
                    };
                    if input.pressed(accelerate_button) && car.throttle < 1.0 {
                        throttle = Throttle::Accelerate;
                    } else if input.pressed(brake_button) && car.throttle > -1.0 {
                        throttle = Throttle::Brake;
                    }
                }
            }
        }

        // apply throttle, consider it takes one second to apply fully.
        match throttle {
            Throttle::Accelerate => {
                let mut t = car.throttle.max(0.0);
                t += time.delta_seconds();
                car.throttle = t.min(1.0);
            }
            Throttle::Brake => {
                let mut t = car.throttle.min(0.0);
                t -= time.delta_seconds();
                car.throttle = t.max(-1.0);
            }
            Throttle::Roll => {
                car.throttle = 0.0;
            }
        }
    }
}

// The gamepad used by the player.
#[derive(Resource)]
struct MyGamepad(Gamepad);

/// Handle gamepad connections.
fn gamepad_connections(
    mut commands: Commands,
    my_gamepad: Option<Res<MyGamepad>>,
    mut gamepad_evr: EventReader<GamepadConnectionEvent>,
) {
    for ev in gamepad_evr.iter() {
        // the ID of the gamepad
        let id = ev.gamepad;
        match &ev.connection {
            GamepadConnection::Connected(info) => {
                println!(
                    "New gamepad connected with ID: {:?}, name: {}",
                    id, info.name
                );

                // if we don't have any gamepad yet, use this one.
                if my_gamepad.is_none() {
                    commands.insert_resource(MyGamepad(id));
                }
            }
            GamepadConnection::Disconnected => {
                println!("Lost gamepad connection with ID: {:?}", id);

                // if it's the one we previously associated with the player,
                // disassociate it:
                if let Some(MyGamepad(old_id)) = my_gamepad.as_deref() {
                    if *old_id == id {
                        commands.remove_resource::<MyGamepad>();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;

    use super::*;

    #[test]
    fn test_wheel_speed() {
        let car = Car {
            direction: Vec2::X,
            throttle: 1.0,
            gear: 1,
            speed: 20.0 / 3.6,
            ..Car::default()
        };
        assert_abs_diff_eq!(
            16.835016,
            wheel_speed(&car, &CORVETTE),
            epsilon = f32::EPSILON
        );
    }

    #[test]
    fn test_engine_rpm() {
        let car = Car {
            direction: Vec2::X,
            throttle: 1.0,
            gear: 1,
            speed: 20.0,
            ..Car::default()
        };
        assert_abs_diff_eq!(
            1476.8217,
            engine_rpm(&car, &CORVETTE, 17.0),
            epsilon = f32::EPSILON
        );
    }

    #[test]
    fn test_model_rpms() {
        assert_abs_diff_eq!(1000.0, CORVETTE.min_rpm());
        //assert_abs_diff_eq!(5800.0, CORVETTE.max_rpm());
        assert_abs_diff_eq!(5000.0, CORVETTE.best_rpm());
        assert_abs_diff_eq!(450.0, lookup_torque(&CORVETTE, 1000.0));
        assert_abs_diff_eq!(462.0, lookup_torque(&CORVETTE, 1200.0));
        assert_abs_diff_eq!(480.0, lookup_torque(&CORVETTE, 1500.0));
        assert_abs_diff_eq!(483.33334, lookup_torque(&CORVETTE, 2000.0));
        assert_abs_diff_eq!(490.0, lookup_torque(&CORVETTE, 3000.0));
        assert_abs_diff_eq!(495.0, lookup_torque(&CORVETTE, 4000.0));
        assert_abs_diff_eq!(500.0, lookup_torque(&CORVETTE, 5000.0));
        assert_abs_diff_eq!(475.0, lookup_torque(&CORVETTE, 5400.0));
        assert_abs_diff_eq!(450.0, lookup_torque(&CORVETTE, 5800.0));
    }
}
