use std::time::Duration;

use bevy::prelude::*;

// ---------- Tuning ----------
const WINDOW_SIZE: Vec2 = Vec2::new(800.0, 400.0);
const GROUND_Y: f32 = -100.0; // top surface of the ground

const PLAYER_X: f32 = -250.0;
const PLAYER_WIDTH: f32 = 40.0;
const STAND_HEIGHT: f32 = 40.0;
const DUCK_HEIGHT: f32 = 20.0;

const JUMP_VELOCITY: f32 = 650.0;
const GRAVITY: f32 = 1800.0;
const FAST_FALL_GRAVITY: f32 = 4500.0; // holding down while airborne

const BASE_SPEED: f32 = 300.0;
const MAX_SPEED: f32 = 700.0;
const SPAWN_X: f32 = WINDOW_SIZE.x / 2.0 + 50.0;
const DESPAWN_X: f32 = -WINDOW_SIZE.x / 2.0 - 100.0;

// Obstacle sizes. The air obstacle's bottom sits above DUCK_HEIGHT but below
// STAND_HEIGHT, so a standing player hits it, a ducking player slides under it,
// and a jumping player clears it.
const CACTUS_SIZE: Vec2 = Vec2::new(30.0, 50.0);
const BIRD_SIZE: Vec2 = Vec2::new(40.0, 25.0);
const BIRD_BOTTOM: f32 = 30.0; // height above ground

const HITBOX_FORGIVENESS: f32 = 4.0; // shrink hitboxes slightly so deaths feel fair

// ---------- State, resources, components ----------
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
enum GameState {
    #[default]
    Playing,
    GameOver,
}

#[derive(Resource, Default)]
struct Score(f32);

impl Score {
    fn speed(&self) -> f32 {
        (BASE_SPEED + self.0 * 0.8).min(MAX_SPEED)
    }
}

#[derive(Resource)]
struct SpawnTimer(Timer);

#[derive(Component, Default)]
struct Player {
    /// Height of the player's feet above the ground.
    height: f32,
    /// Vertical velocity (positive = up).
    velocity: f32,
}

#[derive(Component)]
struct Obstacle {
    size: Vec2,
}

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct GameOverText;

// ---------- App ----------
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Getaway Tandem".into(),
                resolution: WINDOW_SIZE.into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.96, 0.96, 0.96)))
        .init_resource::<Score>()
        .insert_resource(SpawnTimer(Timer::from_seconds(1.0, TimerMode::Once)))
        .init_state::<GameState>()
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Playing), reset_game)
        .add_systems(OnEnter(GameState::GameOver), show_game_over)
        .add_systems(OnExit(GameState::GameOver), hide_game_over)
        .add_systems(
            Update,
            (
                player_control,
                spawn_obstacles,
                move_obstacles,
                update_score,
                check_collisions,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, restart_on_space.run_if(in_state(GameState::GameOver)))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Ground line
    commands.spawn((
        Sprite::from_color(Color::srgb(0.3, 0.3, 0.3), Vec2::new(WINDOW_SIZE.x, 4.0)),
        Transform::from_xyz(0.0, GROUND_Y - 2.0, 0.0),
    ));

    // Player
    commands.spawn((
        Player::default(),
        Sprite::from_color(Color::srgb(0.15, 0.15, 0.15), Vec2::new(PLAYER_WIDTH, STAND_HEIGHT)),
        Transform::from_xyz(PLAYER_X, GROUND_Y + STAND_HEIGHT / 2.0, 1.0),
    ));

    // Score UI
    commands.spawn((
        ScoreText,
        Text::new("Score: 0"),
        TextColor(Color::srgb(0.2, 0.2, 0.2)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(15.0),
            ..default()
        },
    ));
}

// ---------- Systems ----------
fn reset_game(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut timer: ResMut<SpawnTimer>,
    obstacles: Query<Entity, With<Obstacle>>,
    mut player: Query<(&mut Player, &mut Transform, &mut Sprite)>,
) {
    for entity in &obstacles {
        commands.entity(entity).despawn();
    }
    score.0 = 0.0;
    timer.0.set_duration(Duration::from_secs_f32(1.0));
    timer.0.reset();

    for (mut p, mut transform, mut sprite) in &mut player {
        *p = Player::default();
        sprite.custom_size = Some(Vec2::new(PLAYER_WIDTH, STAND_HEIGHT));
        transform.translation.y = GROUND_Y + STAND_HEIGHT / 2.0;
    }
}

fn player_control(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<(&mut Player, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    let jump_held = keys.any_pressed([KeyCode::Space, KeyCode::ArrowUp, KeyCode::KeyW]);
    let down_held = keys.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]);

    for (mut player, mut transform, mut sprite) in &mut query {
        let on_ground = player.height <= 0.0 && player.velocity <= 0.0;

        if on_ground && jump_held {
            player.velocity = JUMP_VELOCITY;
        }

        // Ducking only on the ground; holding down in the air fast-falls instead.
        let ducking = on_ground && down_held && !jump_held;
        let gravity = if down_held && !on_ground {
            FAST_FALL_GRAVITY
        } else {
            GRAVITY
        };

        player.velocity -= gravity * dt;
        player.height += player.velocity * dt;
        if player.height <= 0.0 {
            player.height = 0.0;
            player.velocity = 0.0;
        }

        let h = if ducking { DUCK_HEIGHT } else { STAND_HEIGHT };
        sprite.custom_size = Some(Vec2::new(PLAYER_WIDTH, h));
        transform.translation.y = GROUND_Y + h / 2.0 + player.height;
    }
}

fn spawn_obstacles(mut commands: Commands, time: Res<Time>, mut timer: ResMut<SpawnTimer>) {
    timer.0.tick(time.delta());
    if !timer.0.finished() {
        return;
    }

    if rand::random::<bool>() {
        // Ground obstacle: jump over it.
        commands.spawn((
            Obstacle { size: CACTUS_SIZE },
            Sprite::from_color(Color::srgb(0.2, 0.55, 0.25), CACTUS_SIZE),
            Transform::from_xyz(SPAWN_X, GROUND_Y + CACTUS_SIZE.y / 2.0, 0.5),
        ));
    } else {
        // Air obstacle: jump over it or duck under it.
        commands.spawn((
            Obstacle { size: BIRD_SIZE },
            Sprite::from_color(Color::srgb(0.7, 0.25, 0.25), BIRD_SIZE),
            Transform::from_xyz(SPAWN_X, GROUND_Y + BIRD_BOTTOM + BIRD_SIZE.y / 2.0, 0.5),
        ));
    }

    let next = rand::random_range(0.9..1.8);
    timer.0.set_duration(Duration::from_secs_f32(next));
    timer.0.reset();
}

fn move_obstacles(
    mut commands: Commands,
    time: Res<Time>,
    score: Res<Score>,
    mut query: Query<(Entity, &mut Transform), With<Obstacle>>,
) {
    let dx = score.speed() * time.delta_secs();
    for (entity, mut transform) in &mut query {
        transform.translation.x -= dx;
        if transform.translation.x < DESPAWN_X {
            commands.entity(entity).despawn();
        }
    }
}

fn update_score(
    time: Res<Time>,
    mut score: ResMut<Score>,
    mut text: Query<&mut Text, With<ScoreText>>,
) {
    score.0 += time.delta_secs() * 10.0;
    for mut t in &mut text {
        t.0 = format!("Score: {}", score.0 as u32);
    }
}

fn check_collisions(
    player: Query<(&Transform, &Sprite), With<Player>>,
    obstacles: Query<(&Transform, &Obstacle)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok((p_transform, p_sprite)) = player.single() else {
        return;
    };
    let p_half = p_sprite.custom_size.unwrap_or(Vec2::ONE) / 2.0 - Vec2::splat(HITBOX_FORGIVENESS);
    let p_pos = p_transform.translation.truncate();

    for (o_transform, obstacle) in &obstacles {
        let o_half = obstacle.size / 2.0 - Vec2::splat(HITBOX_FORGIVENESS);
        let o_pos = o_transform.translation.truncate();
        let delta = (p_pos - o_pos).abs();

        if delta.x < p_half.x + o_half.x && delta.y < p_half.y + o_half.y {
            next_state.set(GameState::GameOver);
            return;
        }
    }
}

fn show_game_over(mut commands: Commands) {
    commands
        .spawn((
            GameOverText,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(30.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("GAME OVER - press Space to restart"),
                TextColor(Color::srgb(0.2, 0.2, 0.2)),
            ));
        });
}

fn hide_game_over(mut commands: Commands, query: Query<Entity, With<GameOverText>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn restart_on_space(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<GameState>>) {
    if keys.just_pressed(KeyCode::Space) {
        next_state.set(GameState::Playing);
    }
}