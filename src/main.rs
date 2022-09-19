#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::type_complexity)]
use bevy::{
    prelude::*, render::texture::ImageSettings, sprite::MaterialMesh2dBundle, time::FixedTimestep,
    window, window::PresentMode, window::WindowMode,
};
use heron::{CollisionEvent, CollisionShape, PhysicsPlugin, RigidBody};
use std::{collections::VecDeque, f32::consts::PI};
const BACKGROUND_COLOR: Color = Color::rgb(43. / 255., 32. / 255., 35. / 255.);
const TIME_STEP: f32 = 1.0 / 60.0;
const BIKE_SPEED: f32 = 400.0;
const BIKE_DELTA: f32 = BIKE_SPEED * TIME_STEP;
const TRAIL_BLOCK_HALF: f32 = (BIKE_DELTA / 2.0) - 1.;

const BIKE_WIDTH: f32 = 50.0;
const BIKE_HEIGHT: f32 = 41.0;
const BIKE_WIDTH_CENTER: f32 = 25.0;
const BIKE_HEIGHT_CENTER: f32 = 20.5;

#[derive(Component)]
struct MainCamera;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum AppState {
    InGame,
    Dead,
}

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

#[derive(PartialEq, Eq, Clone, Copy)]
enum Direction {
    Left,
    Right,
    Down,
    Up,
}

#[derive(Component)]
struct Bike {
    direction: Direction,
    atlas_handle: Handle<TextureAtlas>,
}

struct TrailBlock {
    entity: Entity,
    pos: Vec3,
}

#[derive(Component)]
struct Trail {
    tail: VecDeque<TrailBlock>,
    capacity: usize,
}

#[derive(Component)]
struct Player;

impl Trail {
    fn new() -> Self {
        Self {
            capacity: 1000,
            tail: VecDeque::with_capacity(100_000),
        }
    }

    /**
     * Move the trail forward in a direction.
     *
     * To be efficient, we just pop the pack of the trail if the trail is at capacity
     *  and add a new block to the front instead of moving every block.
     * Trailblocks are added to the back of the bike
     */
    fn trek(
        &mut self,
        bike: &mut Bike,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<ColorMaterial>>,
        bike_pos: Vec3,
        commands: &mut Commands,
    ) {
        if self.tail.len() == self.capacity {
            // pop the back of the tail off and despawn it.
            commands
                .entity(self.tail.pop_back().unwrap().entity)
                .despawn();
        }

        let new_head_pos = match bike.direction {
            Direction::Down => Vec3::new(
                bike_pos.x + BIKE_HEIGHT_CENTER,
                bike_pos.y + BIKE_DELTA + BIKE_WIDTH_CENTER,
                0.,
            ),
            Direction::Up => Vec3::new(
                bike_pos.x + BIKE_HEIGHT_CENTER,
                bike_pos.y - BIKE_DELTA - BIKE_WIDTH_CENTER,
                0.,
            ),
            Direction::Left => Vec3::new(
                bike_pos.x + BIKE_DELTA + BIKE_WIDTH_CENTER,
                bike_pos.y - BIKE_HEIGHT_CENTER,
                0.,
            ),
            Direction::Right => Vec3::new(
                bike_pos.x - BIKE_DELTA - BIKE_WIDTH_CENTER,
                bike_pos.y - BIKE_HEIGHT_CENTER,
                0.,
            ),
        };
        // after bike moves there's a gap between bike and trail that we fill.
        let entity = commands
            .spawn_bundle(MaterialMesh2dBundle {
                mesh: meshes.add(Mesh::from(shape::Quad::default())).into(),
                transform: Transform::default()
                    .with_scale(Vec3::new(BIKE_DELTA, BIKE_DELTA, 1.))
                    .with_translation(new_head_pos),
                material: materials.add(ColorMaterial::from(Color::RED)),
                ..default()
            })
            .insert(RigidBody::Static {})
            .insert(CollisionShape::Cuboid {
                half_extends: Vec3::new(TRAIL_BLOCK_HALF, TRAIL_BLOCK_HALF, 1.),
                border_radius: None,
            })
            .id();
        self.tail.push_front(TrailBlock {
            entity,
            pos: new_head_pos,
        });
    }
}

fn main() {
    #![allow(clippy::cast_lossless)]
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(PhysicsPlugin::default())
        .add_state(AppState::InGame)
        .insert_resource(WindowDescriptor {
            title: "Cyber Cycle".to_string(),
            width: 5000.,
            height: 5000.,
            fit_canvas_to_parent: true,
            present_mode: PresentMode::AutoVsync,
            mode: WindowMode::BorderlessFullscreen,
            ..Default::default()
        })
        .insert_resource(ImageSettings::default_nearest()) // prevents blurry sprites
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .add_startup_system(setup)
        .add_system_set(
            SystemSet::on_update(AppState::InGame)
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(check_collisions)
                .with_system(animate_sprite)
                .with_system(camera_system)
                .with_system(player_movement),
        )
        .run();
}

fn animate_sprite(
    time: Res<Time>,
    texture_atlases: Res<Assets<TextureAtlas>>,
    mut query: Query<(
        &mut AnimationTimer,
        &mut TextureAtlasSprite,
        &Handle<TextureAtlas>,
    )>,
) {
    for (mut timer, mut sprite, texture_atlas_handle) in &mut query {
        timer.tick(time.delta());
        if timer.just_finished() {
            let texture_atlas = texture_atlases.get(texture_atlas_handle).unwrap();
            sprite.index = (sprite.index + 1) % texture_atlas.textures.len();
        }
    }
}

fn camera_system(
    mut set: ParamSet<(
        Query<&Transform, With<Player>>,
        Query<&mut Transform, With<Camera>>,
    )>,
) {
    let player_pos = match set.p0().get_single() {
        Ok(transform) => transform.translation,
        Err(_) => return,
    };
    let mut p1 = set.p1();
    let mut camera_pos = p1.get_single_mut().unwrap();

    camera_pos.translation.x = player_pos.x.round();
    camera_pos.translation.y = player_pos.y.round();
}

fn player_movement(
    keyboard_input: Res<Input<KeyCode>>,
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    mut query: Query<(&mut Transform, &mut Bike, &mut Trail), With<Player>>,
) {
    let (bike_transform, mut bike, trail) = match query.get_single_mut() {
        Ok(q) => q,
        Err(_) => return,
    };
    let bike_transform = bike_transform.into_inner();
    let prev_direction = bike.direction;

    if keyboard_input.pressed(KeyCode::Left) && bike.direction != Direction::Right {
        bike.direction = Direction::Left;
    } else if keyboard_input.pressed(KeyCode::Right) && bike.direction != Direction::Left {
        bike.direction = Direction::Right;
    } else if keyboard_input.pressed(KeyCode::Down) && bike.direction != Direction::Up {
        bike.direction = Direction::Down;
    } else if keyboard_input.pressed(KeyCode::Up) && bike.direction != Direction::Down {
        bike.direction = Direction::Up;
    }

    move_bike(
        bike_transform,
        bike,
        trail.into_inner(),
        &mut commands,
        prev_direction,
        meshes,
        materials,
    );
}

fn move_bike(
    bike_transform: &mut Transform,
    bike: Mut<Bike>,
    trail: &mut Trail,
    commands: &mut Commands,
    prev_direction: Direction,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut rotate_point = bike_transform.translation;
    match bike.direction {
        Direction::Left => {
            match prev_direction {
                Direction::Up => {
                    rotate_point.y -= BIKE_WIDTH_CENTER - BIKE_HEIGHT_CENTER;
                    let z_angle = Quat::from_rotation_z(PI / 2.);
                    bike_transform.rotate_around(rotate_point, z_angle);
                    bike_transform.rotate_x(-PI);
                }
                Direction::Down => {
                    rotate_point.x += BIKE_HEIGHT_CENTER;
                    rotate_point.y += BIKE_WIDTH_CENTER;
                    let z_angle = Quat::from_rotation_z(-PI / 2.);
                    bike_transform.rotate_around(rotate_point, z_angle);
                }
                _ => (),
            }
            bike_transform.translation.x -= BIKE_DELTA;
        }
        Direction::Right => {
            match prev_direction {
                Direction::Up => {
                    rotate_point.x += BIKE_HEIGHT_CENTER;
                    rotate_point.y -= BIKE_WIDTH_CENTER;
                    let z_angle = Quat::from_rotation_z(-PI / 2.);
                    bike_transform.rotate_around(rotate_point, z_angle);
                }
                Direction::Down => {
                    rotate_point.y += BIKE_WIDTH_CENTER + BIKE_HEIGHT_CENTER;
                    let z_angle = Quat::from_rotation_z(PI / 2.);
                    bike_transform.rotate_around(rotate_point, z_angle);
                    bike_transform.rotate_x(PI);
                }
                _ => (),
            }
            bike_transform.translation.x += BIKE_DELTA;
        }
        Direction::Down => {
            match prev_direction {
                Direction::Left => {
                    rotate_point.x += BIKE_WIDTH_CENTER;
                    rotate_point.y -= BIKE_HEIGHT_CENTER;
                    let z_angle = Quat::from_rotation_z(PI / 2.);
                    bike_transform.rotate_around(rotate_point, z_angle);
                }
                Direction::Right => {
                    rotate_point.x -= BIKE_WIDTH_CENTER + BIKE_HEIGHT_CENTER;
                    let z_angle = Quat::from_rotation_z(-PI / 2.);
                    bike_transform.rotate_around(rotate_point, z_angle);
                    bike_transform.rotate_y(PI);
                }
                _ => (),
            }
            bike_transform.translation.y -= BIKE_DELTA;
        }
        Direction::Up => {
            match prev_direction {
                Direction::Left => {
                    bike_transform.rotate_x(PI);
                    rotate_point.x += BIKE_WIDTH_CENTER - BIKE_HEIGHT_CENTER;
                    let z_angle = Quat::from_rotation_z(-PI / 2.);
                    bike_transform.rotate_around(rotate_point, z_angle);
                }
                Direction::Right => {
                    rotate_point.x -= BIKE_WIDTH_CENTER;
                    rotate_point.y -= BIKE_HEIGHT_CENTER;
                    let z_angle = Quat::from_rotation_z(PI / 2.);
                    bike_transform.rotate_around(rotate_point, z_angle);
                }
                _ => (),
            }
            bike_transform.translation.y += BIKE_DELTA;
        }
    }

    trail.trek(
        bike.into_inner(),
        meshes,
        materials,
        bike_transform.translation,
        commands,
    );
}

fn handle_wall_collision(
    commands: &mut Commands,
    player_id: Option<Entity>,
    bike_id: Entity,
    app_state: &mut ResMut<State<AppState>>,
) {
    commands.entity(bike_id).despawn();
    if let Some(_id) = player_id {
        app_state.set(AppState::Dead).unwrap();
    }
}

// TODO
fn handle_bike_collision() {}

fn check_collisions(
    mut events: EventReader<CollisionEvent>,
    player_q: Query<Option<Entity>, With<Player>>,
    bikes_q: Query<Entity, With<Bike>>,
    mut app_state: ResMut<State<AppState>>,
    mut commands: Commands,
) {
    let player_id = match player_q.get_single() {
        Ok(id) => id,
        Err(_) => return,
    };
    for event in events.iter() {
        println!("got event");
        let (first, second) = match event {
            CollisionEvent::Stopped(first, second) | CollisionEvent::Started(first, second) => {
                (first, second)
            }
        };
        if bikes_q.contains(first.rigid_body_entity()) {
            if bikes_q.contains(second.rigid_body_entity()) {
                handle_bike_collision();
            } else {
                handle_wall_collision(
                    &mut commands,
                    player_id,
                    first.rigid_body_entity(),
                    &mut app_state,
                );
                break;
            }
        } else {
            handle_wall_collision(
                &mut commands,
                player_id,
                second.rigid_body_entity(),
                &mut app_state,
            );
            break;
        }
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut windows: ResMut<window::Windows>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    // audio: Res<Audio>,
) {
    let window = windows.get_primary_mut().unwrap();
    // window.set_maximized(true);

    let texture_handle = asset_server.load("biker.png");
    let texture_atlas =
        TextureAtlas::from_grid(texture_handle, Vec2::new(BIKE_WIDTH, BIKE_HEIGHT), 2, 1);
    let texture_atlas_handle = texture_atlases.add(texture_atlas);
    // let music = asset_server.load("sounds/cyberpunk.ogg");
    let camera_bundle = Camera2dBundle::new_with_far(3.);
    commands.spawn_bundle(camera_bundle).insert(MainCamera);

    commands
        .spawn_bundle(SpriteSheetBundle {
            texture_atlas: texture_atlas_handle.clone(),
            transform: Transform::from_scale(Vec3::splat(1.))
                .with_translation(Vec2::splat(5.).extend(0.)),
            ..Default::default()
        })
        .insert(AnimationTimer(Timer::from_seconds(0.1, true)))
        .insert(Bike {
            direction: Direction::Right,
            atlas_handle: texture_atlas_handle,
        })
        .insert(RigidBody::Sensor {})
        .insert(CollisionShape::Cuboid {
            half_extends: Vec3::new(BIKE_WIDTH_CENTER, BIKE_HEIGHT_CENTER, 0.),
            border_radius: None,
        })
        .insert(Player {})
        .insert(Trail::new());

    // audio.play(music);
}
