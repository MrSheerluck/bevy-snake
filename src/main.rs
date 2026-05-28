use bevy::prelude::*;
use std::collections::VecDeque;

const GRID_WIDTH: i32 = 20;
const GRID_HEIGHT: i32 = 20;
const CELL_SIZE: f32 = 30.0;
const SNAKE_MOVE_INTERVAL: f32 = 0.15;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
struct GridPosition {
    col: i32,
    row: i32,
}

#[derive(Component)]
struct SnakeSegment;
#[derive(Component)]
struct FoodSprite;
#[derive(Component)]
struct ScoreText;
#[derive(Component)]
struct GameOverText;
#[derive(Component)]
struct ScorePop(Timer);
#[derive(Component)]
struct Particle {
    velocity: Vec2,
    lifetime: Timer,
}

#[derive(Resource)]
struct Snake {
    segments: Vec<GridPosition>,
    direction: Direction,
}
#[derive(Resource)]
struct Food(GridPosition);
#[derive(Resource)]
struct MoveTimer(Timer);
#[derive(Resource)]
struct DirectionQueue(VecDeque<Direction>);
#[derive(Resource)]
struct Score(u32);
#[derive(Resource)]
struct ScreenShake(Timer);
#[derive(Resource)]
struct EatSound(Handle<AudioSource>);
#[derive(Resource)]
struct DieSound(Handle<AudioSource>);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(States, Default, Clone, Eq, PartialEq, Hash, Debug)]
enum GameState {
    #[default]
    Playing,
    GameOver,
}

fn grid_to_world(pos: GridPosition) -> Vec3 {
    let half_w = GRID_WIDTH as f32 * CELL_SIZE / 2.0;
    let half_h = GRID_HEIGHT as f32 * CELL_SIZE / 2.0;
    Vec3::new(
        pos.col as f32 * CELL_SIZE + CELL_SIZE / 2.0 - half_w,
        pos.row as f32 * CELL_SIZE + CELL_SIZE / 2.0 - half_h,
        0.0,
    )
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<GameState>()
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::GameOver), show_game_over)
        .add_systems(OnExit(GameState::GameOver), hide_game_over)
        .add_systems(
            Update,
            (
                snake_input,
                snake_move.run_if(in_state(GameState::Playing)),
                sync_sprites,
                update_score_text,
                update_score_pop,
                update_particles,
                apply_screenshake,
                game_over_restart.run_if(in_state(GameState::GameOver)),
            ),
        )
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(EatSound(asset_server.load("audio/eat.wav")));
    commands.insert_resource(DieSound(asset_server.load("audio/die.wav")));

    commands.insert_resource(ScreenShake({
        let mut t = Timer::from_seconds(0.3, TimerMode::Once);
        t.finish();
        t
    }));

    commands.spawn(Camera2d);

    let start_col = GRID_WIDTH / 2;
    let start_row = GRID_HEIGHT / 2;

    let snake = Snake {
        segments: vec![
            GridPosition {
                col: start_col,
                row: start_row,
            },
            GridPosition {
                col: start_col - 1,
                row: start_row,
            },
            GridPosition {
                col: start_col - 2,
                row: start_row,
            },
        ],
        direction: Direction::Right,
    };

    for &pos in &snake.segments {
        commands.spawn((
            SnakeSegment,
            Sprite::from_color(Color::srgb(0.2, 0.8, 0.2), Vec2::splat(CELL_SIZE - 2.0)),
            Transform::from_translation(grid_to_world(pos)),
        ));
    }
    commands.insert_resource(snake);

    let food_pos = GridPosition { col: 15, row: 10 };
    commands.spawn((
        FoodSprite,
        Sprite::from_color(Color::srgb(0.8, 0.2, 0.2), Vec2::splat(CELL_SIZE - 2.0)),
        Transform::from_translation(grid_to_world(food_pos)),
    ));
    commands.insert_resource(Food(food_pos));

    commands.insert_resource(MoveTimer(Timer::from_seconds(
        SNAKE_MOVE_INTERVAL,
        TimerMode::Repeating,
    )));
    commands.insert_resource(DirectionQueue(VecDeque::new()));
    commands.insert_resource(Score(0));

    commands.spawn((
        ScoreText,
        Text2d::new("Score: 0"),
        TextFont {
            font_size: 30.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(0.0, 320.0, 0.0),
    ));
}

fn random_grid_position() -> GridPosition {
    GridPosition {
        col: rand::random::<i32>().rem_euclid(GRID_WIDTH),
        row: rand::random::<i32>().rem_euclid(GRID_HEIGHT),
    }
}

fn spawn_food_on_empty(snake: &Snake) -> GridPosition {
    loop {
        let pos = random_grid_position();
        if !snake.segments.contains(&pos) {
            return pos;
        }
    }
}

fn snake_input(keyboard: Res<ButtonInput<KeyCode>>, mut queue: ResMut<DirectionQueue>) {
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        queue.0.push_back(Direction::Up);
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        queue.0.push_back(Direction::Down);
    } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
        queue.0.push_back(Direction::Left);
    } else if keyboard.just_pressed(KeyCode::ArrowRight) {
        queue.0.push_back(Direction::Right);
    }
}

fn is_opposite(a: Direction, b: Direction) -> bool {
    matches!(
        (a, b),
        (Direction::Up, Direction::Down)
            | (Direction::Down, Direction::Up)
            | (Direction::Left, Direction::Right)
            | (Direction::Right, Direction::Left)
    )
}

fn snake_move(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<MoveTimer>,
    mut snake: ResMut<Snake>,
    mut queue: ResMut<DirectionQueue>,
    mut food: ResMut<Food>,
    mut score: ResMut<Score>,
    mut next_state: ResMut<NextState<GameState>>,
    eat_sound: Res<EatSound>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    while let Some(next_dir) = queue.0.pop_front() {
        if !is_opposite(next_dir, snake.direction) {
            snake.direction = next_dir;
            break;
        }
    }

    let head = snake.segments[0];
    let new_head = match snake.direction {
        Direction::Up => GridPosition {
            col: head.col,
            row: head.row + 1,
        },
        Direction::Down => GridPosition {
            col: head.col,
            row: head.row - 1,
        },
        Direction::Left => GridPosition {
            col: head.col - 1,
            row: head.row,
        },
        Direction::Right => GridPosition {
            col: head.col + 1,
            row: head.row,
        },
    };

    if new_head.col < 0
        || new_head.col >= GRID_WIDTH
        || new_head.row < 0
        || new_head.row >= GRID_HEIGHT
    {
        next_state.set(GameState::GameOver);
        return;
    }

    if snake.segments.contains(&new_head) {
        next_state.set(GameState::GameOver);
        return;
    }

    snake.segments.insert(0, new_head);

    if new_head == food.0 {
        score.0 += 1;
        commands.spawn(AudioPlayer(eat_sound.0.clone()));
        spawn_eat_particles(&mut commands, grid_to_world(new_head));
        food.0 = spawn_food_on_empty(&snake);
    } else {
        snake.segments.pop();
    }
}

fn sync_sprites(
    mut commands: Commands,
    snake: Res<Snake>,
    food: Res<Food>,
    segments: Query<Entity, With<SnakeSegment>>,
    food_sprites: Query<Entity, With<FoodSprite>>,
) {
    for entity in &segments {
        commands.entity(entity).despawn();
    }
    for entity in &food_sprites {
        commands.entity(entity).despawn();
    }

    for (i, &pos) in snake.segments.iter().enumerate() {
        let color = if i == 0 {
            Color::srgb(0.0, 0.95, 0.0)
        } else {
            Color::srgb(0.2, 0.7, 0.2)
        };
        commands.spawn((
            SnakeSegment,
            Sprite::from_color(color, Vec2::splat(CELL_SIZE - 2.0)),
            Transform::from_translation(grid_to_world(pos)),
        ));
    }

    commands.spawn((
        FoodSprite,
        Sprite::from_color(Color::srgb(0.9, 0.2, 0.2), Vec2::splat(CELL_SIZE - 2.0)),
        Transform::from_translation(grid_to_world(food.0)),
    ));
}

fn update_score_text(
    score: Res<Score>,
    mut query: Query<&mut Text2d, With<ScoreText>>,
    mut commands: Commands,
) {
    for mut text in &mut query {
        let old = text.0.clone();
        text.0 = format!("Score: {}", score.0);
        if old != text.0 {
            commands.spawn((
                ScorePop(Timer::from_seconds(0.3, TimerMode::Once)),
                Text2d::new(text.0.clone()),
                TextFont {
                    font_size: 44.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 0.3)),
                Transform::from_xyz(0.0, 320.0, 2.0),
            ));
        }
    }
}

fn update_score_pop(
    mut commands: Commands,
    time: Res<Time>,
    mut pops: Query<(Entity, &mut ScorePop, &mut Transform, &mut TextColor)>,
) {
    for (entity, mut pop, mut transform, mut color) in &mut pops {
        pop.0.tick(time.delta());
        if pop.0.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        let t = pop.0.fraction_remaining();
        transform.translation.y = 320.0 + (1.0 - t) * 30.0;
        color.0.set_alpha(t);
    }
}

fn game_over_restart(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut snake: ResMut<Snake>,
    mut food: ResMut<Food>,
    mut score: ResMut<Score>,
    mut timer: ResMut<MoveTimer>,
    mut queue: ResMut<DirectionQueue>,
) {
    if !keyboard.just_pressed(KeyCode::Enter) {
        return;
    }

    let start_col = GRID_WIDTH / 2;
    let start_row = GRID_HEIGHT / 2;
    snake.segments = vec![
        GridPosition {
            col: start_col,
            row: start_row,
        },
        GridPosition {
            col: start_col - 1,
            row: start_row,
        },
        GridPosition {
            col: start_col - 2,
            row: start_row,
        },
    ];
    snake.direction = Direction::Right;
    food.0 = GridPosition { col: 15, row: 10 };
    score.0 = 0;
    timer.0.reset();
    queue.0.clear();

    next_state.set(GameState::Playing);
}

fn show_game_over(
    mut commands: Commands,
    die_sound: Res<DieSound>,
    mut shake: ResMut<ScreenShake>,
) {
    commands.spawn(AudioPlayer(die_sound.0.clone()));
    shake.0.reset();
    commands.spawn((
        GameOverText,
        Text2d::new("Game Over\nPress Enter to Restart"),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.2, 0.2)),
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));
}

fn hide_game_over(mut commands: Commands, query: Query<Entity, With<GameOverText>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn spawn_eat_particles(commands: &mut Commands, position: Vec3) {
    for _ in 0..10 {
        let angle = rand::random::<f32>() * std::f32::consts::TAU;
        let speed = 60.0 + rand::random::<f32>() * 80.0;
        commands.spawn((
            Particle {
                velocity: Vec2::new(angle.cos() * speed, angle.sin() * speed),
                lifetime: Timer::from_seconds(0.6, TimerMode::Once),
            },
            Sprite::from_color(Color::srgb(1.0, 0.9, 0.2), Vec2::splat(5.0)),
            Transform::from_translation(position),
        ));
    }
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>,
) {
    for (entity, mut particle, mut transform, mut sprite) in &mut particles {
        particle.lifetime.tick(time.delta());
        if particle.lifetime.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        let t = particle.lifetime.fraction_remaining();
        transform.translation.x += particle.velocity.x * time.delta_secs();
        transform.translation.y += particle.velocity.y * time.delta_secs();
        sprite.color.set_alpha(t);
        transform.scale = Vec3::splat(0.5 + t * 0.5);
    }
}

fn apply_screenshake(
    mut shake: ResMut<ScreenShake>,
    mut camera: Query<&mut Transform, With<Camera2d>>,
    time: Res<Time>,
) {
    shake.0.tick(time.delta());
    for mut transform in &mut camera {
        if shake.0.is_finished() {
            if transform.translation != Vec3::ZERO {
                transform.translation = Vec3::ZERO;
            }
        } else {
            let intensity = 10.0 * (1.0 - shake.0.fraction());
            transform.translation = Vec3::new(
                rand::random::<f32>() * 2.0 * intensity - intensity,
                rand::random::<f32>() * 2.0 * intensity - intensity,
                0.0,
            );
        }
    }
}
