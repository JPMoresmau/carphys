use bevy::prelude::*;
use carphys::{Car, CarPlugin, Player};

/// App entry point.
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(CarPlugin)
        .add_startup_system(setup_graphics)
        .add_system(update_speed)
        .add_system(show_gear)
        .add_system(show_rpm)
        .run();
}

/// Setup the graphics: camera, texts and pedal icons.
fn setup_graphics(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Add a camera so we can see the debug-render.
    commands.spawn(Camera2dBundle::default());
    commands
        .spawn(NodeBundle {
            style: Style {
                size: Size::width(Val::Percent(100.0)),
                align_items: AlignItems::Start,
                justify_content: JustifyContent::SpaceEvenly,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                TextBundle::from_section(
                    "",
                    TextStyle {
                        font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                        font_size: 60.0,
                        color: Color::GOLD,
                    },
                )
                .with_text_alignment(TextAlignment::Center),
                SpeedDial,
            ));

            parent.spawn((
                TextBundle::from_section(
                    "",
                    TextStyle {
                        font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                        font_size: 60.0,
                        color: Color::GOLD,
                    },
                )
                .with_text_alignment(TextAlignment::Center),
                GearDial,
            ));

            parent.spawn((
                TextBundle::from_section(
                    "",
                    TextStyle {
                        font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                        font_size: 60.0,
                        color: Color::GOLD,
                    },
                )
                .with_text_alignment(TextAlignment::Center),
                RpmDial,
            ));
        });

    commands.spawn(SpriteBundle {
        texture: asset_server.load("icons/car-pedals.png"),
        transform: Transform {
            translation: Vec3::new(0.0, -240.0, 0.0),
            scale: Vec3::new(0.5, 0.5, 1.0),
            ..default()
        },
        ..default()
    });
}

/// Mark the speed dial text bundle.
#[derive(Component)]
struct SpeedDial;

/// Mark the gear dial text bundle.
#[derive(Component)]
struct GearDial;

/// Mark the rpm dial text bundle.
#[derive(Component)]
struct RpmDial;

/// Show the current speed.
fn update_speed(
    mut speed_text: Query<&mut Text, With<SpeedDial>>,
    cars: Query<&Car, With<Player>>,
) {
    if let Ok(car) = cars.get_single() {
        for mut text in &mut speed_text {
            text.sections[0].value = format!("{:.2} KM/H", car.speed * 3.6);
        }
    }
}

/// Show the current gear.
fn show_gear(mut speed_text: Query<&mut Text, With<GearDial>>, cars: Query<&Car, With<Player>>) {
    if let Ok(car) = cars.get_single() {
        for mut text in &mut speed_text {
            text.sections[0].value = format!("Gear {}", car.gear);
        }
    }
}

/// Show the current RPM.
fn show_rpm(mut speed_text: Query<&mut Text, With<RpmDial>>, cars: Query<&Car, With<Player>>) {
    if let Ok(car) = cars.get_single() {
        for mut text in &mut speed_text {
            text.sections[0].value = format!("{:.0} RPM", car.rpm);
        }
    }
}
