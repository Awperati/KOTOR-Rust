//! The swoop race, `Type` 1 of an area's `MiniGame` struct, which is read here whole. The turret is `minigame.rs`.
//!
//! The race's rules are the area's own scripts, run through the `SWMG_` routines: they shift the gears, set the speeds,
//! hold the clock and end the race. What is here is what they call on: a bike on a track, what it runs into, and which
//! of the models' animations they asked for.
//!
//! From the reference video at 14,681 s, see `docs/REFERENCE.md`: a shift sets the lowest speed without lifting the bike
//! up to it, and the bike covers about one layout unit a second for each point of speed.
//!
//! **Assumptions, not measured:** a bike touches a pad or a buoy when its `Sphere_Radius` reaches the box of the model's
//! `hitbump`. Across the track it gathers speed at the lateral acceleration, loses it at the same rate when let go, and
//! goes no faster than [`LATERAL_TOP`]. A speed above the highest goes down by its excess each second.

use crate::world::{ObjId, World};
use kotor_core::{ResType, Result};
use kotor_gff::{Gff, Struct};
use kotor_script::{Value, OBJECT_INVALID, OBJECT_SELF};
use std::collections::{BTreeMap, HashMap};

/// Followers and obstacles are not world objects. Scripts hold these numbers for them.
const FOLLOWER_BASE: ObjId = 0x7E00_0000;
const OBSTACLE_BASE: ObjId = 0x7F00_0000;
/// Fastest a bike goes across the track, in layout units a second.
const LATERAL_TOP: f32 = 100.0;

/// One bank of guns, as `MiniGame` has it. The turret's; a race's are empty models.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GunBankFile {
    pub bank: i64,
    pub gun_model: String,
    pub fire_sound: String,
    pub damage: i64,
    pub lifespan: f32,
    pub bullet_model: String,
    pub rate_of_fire: f32,
    pub collision_sound: String,
    pub speed: f32,
    pub target_type: i64,
}

/// The player or an enemy, in the file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FollowerFile {
    pub track: String,
    pub loops: i64,
    pub radius: f32,
    pub invincible_for: f32,
    pub hit_points: i64,
    pub max_hit_points: i64,
    pub bump_damage: i64,
    pub death_sound: String,
    pub engine_sound: String,
    pub models: Vec<String>,
    pub scripts: BTreeMap<String, String>,
    pub trigger: bool,
    pub guns: Vec<GunBankFile>,
}

/// What only the player's struct has.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerExtras {
    pub camera: String,
    pub camera_rotate: bool,
    pub min_speed: f32,
    pub max_speed: f32,
    pub accel_seconds: f32,
    pub start_offset: [f32; 3],
    pub tunnel_infinite: [f32; 3],
    pub tunnel_pos: [f32; 3],
    pub tunnel_neg: [f32; 3],
    pub target_offset: [f32; 3],
}

/// An obstacle, in the file. Where it stands is in the area's layout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObstacleFile {
    pub name: String,
    pub scripts: BTreeMap<String, String>,
}

/// The `MiniGame` struct of an area's `.are`, whole.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MiniGameFile {
    /// 1 a swoop race, 2 the Ebon Hawk's turret, by the models' names.
    pub kind: i64,
    pub near_clip: f32,
    pub far_clip: f32,
    pub view_angle: f32,
    pub bump_plane: i64,
    pub use_inertia: bool,
    pub do_bumping: bool,
    pub dof: i64,
    pub movement_per_second: f32,
    pub lateral_accel: f32,
    pub music: String,
    pub mouse_axes: [i64; 2],
    pub mouse_flip: [bool; 2],
    pub player: FollowerFile,
    pub extras: PlayerExtras,
    pub enemies: Vec<FollowerFile>,
    pub obstacles: Vec<ObstacleFile>,
}

fn text(s: &Struct, label: &str) -> String {
    s.str(label).unwrap_or_default().to_ascii_lowercase()
}

fn number(s: &Struct, label: &str) -> f32 {
    s.float(label).unwrap_or(0.0) as f32
}

fn whole(s: &Struct, label: &str) -> i64 {
    s.int(label).unwrap_or(0)
}

fn scripts_of(s: &Struct) -> BTreeMap<String, String> {
    s.strukt("Scripts")
        .map(|scripts| {
            scripts
                .fields
                .iter()
                .filter_map(|(label, _)| {
                    let name = scripts.str(label)?;
                    (!name.is_empty()).then(|| (label.clone(), name.to_ascii_lowercase()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn follower(s: &Struct) -> FollowerFile {
    let sounds = s.strukt("Sounds");
    let sound = |label: &str| sounds.map(|o| text(o, label)).unwrap_or_default();
    FollowerFile {
        track: text(s, "Track"),
        loops: whole(s, "Num_Loops"),
        radius: number(s, "Sphere_Radius"),
        invincible_for: number(s, "Invince_Period"),
        hit_points: whole(s, "Hit_Points"),
        max_hit_points: whole(s, "Max_HPs"),
        bump_damage: whole(s, "Bump_Damage"),
        death_sound: sound("Death"),
        engine_sound: sound("Engine"),
        models: s
            .list("Models")
            .unwrap_or_default()
            .iter()
            .map(|m| text(m, "Model"))
            .collect(),
        scripts: scripts_of(s),
        trigger: whole(s, "Trigger") != 0,
        guns: s
            .list("Gun_Banks")
            .unwrap_or_default()
            .iter()
            .map(|g| {
                let bullet = g.strukt("Bullet");
                let of = |f: &dyn Fn(&Struct) -> f32| bullet.map_or(0.0, f);
                GunBankFile {
                    bank: whole(g, "BankID"),
                    gun_model: text(g, "Gun_Model"),
                    fire_sound: text(g, "Fire_Sound"),
                    damage: bullet.map_or(0, |b| whole(b, "Damage")),
                    lifespan: of(&|b| number(b, "Lifespan")),
                    bullet_model: bullet.map(|b| text(b, "Bullet_Model")).unwrap_or_default(),
                    rate_of_fire: of(&|b| number(b, "Rate_Of_Fire")),
                    collision_sound: bullet
                        .map(|b| text(b, "Collision_Sound"))
                        .unwrap_or_default(),
                    speed: of(&|b| number(b, "Speed")),
                    target_type: bullet.map_or(0, |b| whole(b, "Target_Type")),
                }
            })
            .collect(),
    }
}

impl MiniGameFile {
    /// What an area's file says, when it holds a minigame.
    pub fn of_area(are: &Gff) -> Option<MiniGameFile> {
        let m = are.root.strukt("MiniGame")?;
        let kind = whole(m, "Type");
        if kind == 0 {
            return None;
        }
        let player = m.strukt("Player")?;
        let triple =
            |a: &str, b: &str, c: &str| [number(player, a), number(player, b), number(player, c)];
        let mouse = m.strukt("Mouse");
        let flip = |label: &str| mouse.is_some_and(|o| whole(o, label) != 0);
        let infinite = match player.get("TunnelInfinite") {
            Some(kotor_gff::Value::Vector(v)) => *v,
            _ => [0.0; 3],
        };
        Some(MiniGameFile {
            kind,
            near_clip: number(m, "Near_Clip"),
            far_clip: number(m, "Far_Clip"),
            view_angle: number(m, "CameraViewAngle"),
            bump_plane: whole(m, "Bump_Plane"),
            use_inertia: whole(m, "UseInertia") != 0,
            do_bumping: whole(m, "DoBumping") != 0,
            dof: whole(m, "DOF"),
            movement_per_second: number(m, "MovementPerSec"),
            lateral_accel: number(m, "LateralAccel"),
            music: text(m, "Music"),
            mouse_axes: [
                mouse.map_or(0, |o| whole(o, "AxisX")),
                mouse.map_or(0, |o| whole(o, "AxisY")),
            ],
            mouse_flip: [flip("FlipAxisX"), flip("FlipAxisY")],
            player: follower(player),
            extras: PlayerExtras {
                camera: text(player, "Camera"),
                camera_rotate: whole(player, "CameraRotate") != 0,
                min_speed: number(player, "Minimum_Speed"),
                max_speed: number(player, "Maximum_Speed"),
                accel_seconds: number(player, "Accel_Secs"),
                start_offset: triple("Start_Offset_X", "Start_Offset_Y", "Start_Offset_Z"),
                tunnel_infinite: infinite,
                tunnel_pos: triple("TunnelXPos", "TunnelYPos", "TunnelZPos"),
                tunnel_neg: triple("TunnelXNeg", "TunnelYNeg", "TunnelZNeg"),
                target_offset: triple("Target_Offset_X", "Target_Offset_Y", "Target_Offset_Z"),
            },
            enemies: m
                .list("Enemies")
                .unwrap_or_default()
                .iter()
                .map(follower)
                .collect(),
            obstacles: m
                .list("Obstacles")
                .unwrap_or_default()
                .iter()
                .map(|o| ObstacleFile {
                    name: text(o, "Name"),
                    scripts: scripts_of(o),
                })
                .collect(),
        })
    }
}

/// A box on the floor, in layout units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitBox {
    pub centre: [f32; 2],
    pub half: [f32; 2],
}

impl HitBox {
    fn reached(&self, at: [f32; 3], radius: f32) -> bool {
        (at[0] - self.centre[0]).abs() <= self.half[0] + radius
            && (at[1] - self.centre[1]).abs() <= self.half[1] + radius
    }
}

/// A pad or anything else that follows its own track.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rider {
    pub models: Vec<String>,
    pub at: [f32; 3],
    pub hit_points: i64,
    pub max_hit_points: i64,
    pub trigger: bool,
    pub death_sound: String,
    /// Seconds into the race it fell.
    pub fell_at: Option<f32>,
    pub hit_box: HitBox,
    touching: bool,
}

/// A buoy.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Obstacle {
    pub model: String,
    pub at: [f32; 3],
    pub hit_box: HitBox,
    touching: bool,
}

/// An animation a script has asked for. Which of the bike's models hold it is for whoever draws them.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayingAnimation {
    pub name: String,
    pub looping: bool,
    pub overlay: bool,
    /// Race seconds it starts at.
    pub from: f32,
    pub length: f32,
}

/// A race under way.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Race {
    pub file: MiniGameFile,
    /// Seconds since it began.
    pub time: f32,
    /// Where the track puts the bike at each of its keys: seconds and place.
    path: Vec<(f32, [f32; 3])>,
    /// How far along the track's animation it is, in seconds of that animation.
    along: f32,
    pub offset: [f32; 3],
    pub(crate) across_speed: f32,
    pub speed: f32,
    pub min_speed: f32,
    pub max_speed: f32,
    pub accel: f32,
    pub lateral_accel: f32,
    pub tunnel_pos: [f32; 3],
    pub tunnel_neg: [f32; 3],
    pub invincibility: f32,
    invulnerable_until: f32,
    pub origin: [f32; 3],
    pub riders: Vec<Rider>,
    pub obstacles: Vec<Obstacle>,
    pub animations: Vec<PlayingAnimation>,
    /// Lengths of what the bike's models can play, by lower-case name.
    lengths: HashMap<String, f32>,
    /// The speed blur's ratio, when it is on.
    pub blur: Option<f32>,
    last_rider_hit: Option<usize>,
    last_obstacle_hit: Option<usize>,
    /// This step: which way the player steers, -1 to 1, rightward.
    pub steer: f32,
    /// The player pressed fire.
    pub fire: bool,
    created: bool,
}

/// Where a model's `hitbump` lies, relative to the model. Its mesh is not drawn.
fn hit_box_of(model: &kotor_model::Model, radius: f32) -> HitBox {
    let Some(node) = model.root.find("hitbump") else {
        return HitBox {
            centre: [0.0; 2],
            half: [radius; 2],
        };
    };
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in node.mesh.iter().flat_map(|m| &m.positions) {
        for i in 0..2 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    if lo[0] > hi[0] {
        return HitBox {
            centre: [node.position[0], node.position[1]],
            half: [radius; 2],
        };
    }
    HitBox {
        centre: [0, 1].map(|i| node.position[i] + (lo[i] + hi[i]) / 2.0),
        half: [0, 1].map(|i| (hi[i] - lo[i]) / 2.0),
    }
}

impl Race {
    /// Where the bike is.
    pub fn position(&self) -> [f32; 3] {
        let on_track = self.path_at(self.along);
        [0, 1, 2].map(|i| on_track[i] + self.offset[i])
    }

    fn path_at(&self, t: f32) -> [f32; 3] {
        let Some(first) = self.path.first() else {
            return self.origin;
        };
        let mut before = *first;
        for key in &self.path {
            if key.0 >= t {
                let span = key.0 - before.0;
                let share = if span > 0.0 {
                    (t - before.0) / span
                } else {
                    0.0
                };
                return [0, 1, 2].map(|i| before.1[i] + (key.1[i] - before.1[i]) * share);
            }
            before = *key;
        }
        before.1
    }

    fn track_length(&self) -> f32 {
        self.path.last().map_or(0.0, |k| k.0)
    }

    /// Ask for an animation by name. One that is on already starts over: the clock's digits come every step.
    fn play(&mut self, name: &str, looping: bool, queue: bool, overlay: bool) {
        let key = name.to_ascii_lowercase();
        let Some(length) = self.lengths.get(&key).copied() else {
            return;
        };
        let mut from = self.time;
        if queue {
            if let Some(last) = self.animations.iter().rev().find(|a| a.overlay == overlay) {
                from = last.from + last.length;
            }
        } else if !overlay {
            self.animations.retain(|a| a.overlay);
        }
        self.animations.retain(|a| a.name != key);
        self.animations.push(PlayingAnimation {
            name: key,
            looping,
            overlay,
            from,
            length,
        });
    }

    /// Whatever is on now, with how far into it, base animations first.
    pub fn animations_now(&self) -> Vec<(String, f32)> {
        let on = |a: &&PlayingAnimation| a.from <= self.time;
        let at = |a: &PlayingAnimation| {
            let t = self.time - a.from;
            let t = if a.looping && a.length > 0.0 {
                t % a.length
            } else {
                t
            };
            (a.name.clone(), t)
        };
        let base = self.animations.iter().filter(|a| !a.overlay).filter(on);
        let over = self.animations.iter().filter(|a| a.overlay).filter(on);
        base.chain(over).map(at).collect()
    }

    /// Time passes: the bike moves, and what has run its course goes.
    fn advance(&mut self, dt: f32) {
        self.time += dt;
        let now = self.time;
        self.animations
            .retain(|a| a.looping || now <= a.from + a.length);
        if self.speed < self.max_speed {
            self.speed = (self.speed + self.accel * dt).min(self.max_speed);
        } else if self.speed > self.max_speed {
            self.speed -= (self.speed - self.max_speed) * dt.min(1.0);
        }
        let per_second = self.file.movement_per_second.max(1.0);
        self.along += dt * self.speed / per_second;
        let length = self.track_length();
        if length > 0.0 && self.along >= length {
            self.along = if self.file.player.loops < 0 {
                self.along % length
            } else {
                length
            };
        }
        // Across the track.
        let l = self.lateral_accel;
        if self.steer != 0.0 {
            self.across_speed += self.steer * l * dt;
        } else {
            let slow = l * dt;
            self.across_speed -= self.across_speed.signum() * slow.min(self.across_speed.abs());
        }
        self.across_speed = self.across_speed.clamp(-LATERAL_TOP, LATERAL_TOP);
        let x = self.offset[0] + self.across_speed * dt;
        let (low, high) = (
            self.tunnel_neg[0],
            self.tunnel_pos[0].max(self.tunnel_neg[0]),
        );
        self.offset[0] = x.clamp(low, high);
        if x != self.offset[0] {
            self.across_speed = 0.0;
        }
    }

    /// Which followers and obstacles it has just reached.
    fn newly_touched(&mut self) -> (Vec<usize>, Vec<usize>) {
        let at = self.position();
        let radius = self.file.player.radius;
        let mut riders = Vec::new();
        for (i, r) in self.riders.iter_mut().enumerate() {
            let touching = r.fell_at.is_none() && r.hit_box.reached(at, radius);
            if touching && !r.touching {
                riders.push(i);
            }
            r.touching = touching;
        }
        let mut obstacles = Vec::new();
        for (i, o) in self.obstacles.iter_mut().enumerate() {
            let touching = o.hit_box.reached(at, radius);
            if touching && !o.touching {
                obstacles.push(i);
            }
            o.touching = touching;
        }
        (riders, obstacles)
    }

    /// Whether the bike can't be hurt just now.
    fn invulnerable(&self) -> bool {
        self.time < self.invulnerable_until
    }
}

impl World {
    fn read_model(&mut self, name: &str) -> Option<kotor_model::Model> {
        let mdl = self.inst.read(name, ResType::MDL).ok()?;
        let mdx = self.inst.read(name, ResType::MDX).unwrap_or_default();
        kotor_model::parse_model(&mdl, &mdx).ok()
    }

    /// Set up what an area's file says, when it holds a race.
    pub(crate) fn load_race(&mut self, are: &Gff, area: &str) -> Result<()> {
        self.race = None;
        let Some(file) = MiniGameFile::of_area(are).filter(|f| f.kind == 1) else {
            return Ok(());
        };
        let layout = kotor_walkmesh::parse_lyt(&self.inst.read(area, ResType::LYT)?)?;
        let place_of = |places: &[kotor_walkmesh::Room], name: &str| {
            places
                .iter()
                .find(|p| p.model.eq_ignore_ascii_case(name))
                .map_or([0.0; 3], |p| p.position)
        };
        let mut game = Race {
            lateral_accel: file.lateral_accel,
            tunnel_pos: file.extras.tunnel_pos,
            tunnel_neg: file.extras.tunnel_neg,
            min_speed: file.extras.min_speed,
            max_speed: file.extras.max_speed,
            ..Race::default()
        };
        let place = place_of(&layout.track_places, &file.player.track);
        game.origin = place;
        if let Some(track) = self.read_model(&file.player.track) {
            let hook = track.root.find("modelhook");
            let rest = hook.map_or([0.0; 3], |h| h.position);
            let at = |key: [f32; 3]| [0, 1, 2].map(|i| place[i] + rest[i] + key[i]);
            game.origin = at([0.0; 3]);
            let controller = track.animation("track").and_then(|a| {
                a.root
                    .find("modelhook")?
                    .controllers
                    .iter()
                    .find(|c| c.kind == 8)
                    .cloned()
            });
            if let Some(c) = controller {
                let row = (c.columns & 0xf).max(1) as usize;
                game.path = c
                    .times
                    .iter()
                    .enumerate()
                    .filter_map(|(i, t)| {
                        let v = c.values.get(i * row..i * row + 3)?;
                        Some((*t, at([v[0], v[1], v[2]])))
                    })
                    .collect();
            }
        }
        for name in file.player.models.clone() {
            if let Some(m) = self.read_model(&name) {
                for a in &m.animations {
                    game.lengths
                        .entry(a.name.to_ascii_lowercase())
                        .or_insert(a.length);
                }
            }
        }
        for e in &file.enemies {
            let at = place_of(&layout.track_places, &e.track);
            let shape = e.models.first().and_then(|m| self.read_model(m));
            let local = shape.as_ref().map_or(
                HitBox {
                    centre: [0.0; 2],
                    half: [e.radius; 2],
                },
                |m| hit_box_of(m, e.radius),
            );
            game.riders.push(Rider {
                models: e.models.clone(),
                at,
                hit_points: e.hit_points,
                max_hit_points: e.max_hit_points,
                trigger: e.trigger,
                death_sound: e.death_sound.clone(),
                fell_at: None,
                hit_box: HitBox {
                    centre: [local.centre[0] + at[0], local.centre[1] + at[1]],
                    half: local.half,
                },
                touching: false,
            });
        }
        for o in &file.obstacles {
            let at = place_of(&layout.obstacle_places, &o.name);
            let local = self
                .read_model(&o.name)
                .map_or(HitBox::default(), |m| hit_box_of(&m, 1.0));
            game.obstacles.push(Obstacle {
                model: o.name.clone(),
                at,
                hit_box: HitBox {
                    centre: [local.centre[0] + at[0], local.centre[1] + at[1]],
                    half: local.half,
                },
                touching: false,
            });
        }
        game.file = file;
        self.race = Some(game);
        Ok(())
    }

    /// Whether a race is on.
    pub fn in_race(&self) -> bool {
        self.race.is_some()
    }

    fn race_script(&mut self, event: &str) {
        let Some(script) = self
            .race
            .as_ref()
            .and_then(|g| g.file.player.scripts.get(event).cloned())
        else {
            return;
        };
        let pc = self.pc;
        if let Err(e) = self.run_script(&script, pc) {
            self.errors.push(format!("{script}: {e}"));
        }
    }

    /// One step of a race.
    pub(crate) fn update_race(&mut self, dt: f32) {
        let Some(game) = self.race.as_mut() else {
            return;
        };
        let create = !std::mem::replace(&mut game.created, true);
        let fire = std::mem::take(&mut game.fire);
        game.advance(dt);
        let (riders, obstacles) = game.newly_touched();
        if create {
            self.race_script("OnCreate");
        }
        if fire {
            self.race_script("OnFire");
        }
        for r in riders {
            if let Some(g) = self.race.as_mut() {
                g.last_rider_hit = Some(r);
            }
            self.race_script("OnHitFollower");
        }
        for o in obstacles {
            if let Some(g) = self.race.as_mut() {
                g.last_obstacle_hit = Some(o);
            }
            self.race_script("OnHitObstacle");
        }
        self.race_script("OnHeartbeat");
    }

    /// The `SWMG_` routines. What this world does not hold answers nothing, and shows up with the other defaults.
    pub(crate) fn swmg_race(&mut self, name: &str, caller: ObjId, args: &[Value]) -> Option<Value> {
        let pc = self.pc;
        let game = self.race.as_mut()?;
        let float = |i: usize| match args.get(i) {
            Some(Value::Float(v)) => *v,
            Some(Value::Int(v)) => *v as f32,
            _ => 0.0,
        };
        let int = |i: usize| match args.get(i) {
            Some(Value::Int(v)) => *v,
            _ => 0,
        };
        let text = |i: usize| match args.get(i) {
            Some(Value::Str(s)) => s.clone(),
            _ => String::new(),
        };
        let vector = |i: usize| match args.get(i) {
            Some(Value::Vector(v)) => *v,
            _ => [0.0; 3],
        };
        let object = |i: usize| match args.get(i) {
            Some(Value::Object(OBJECT_SELF)) | None => caller,
            Some(Value::Object(o)) => *o,
            _ => OBJECT_INVALID,
        };
        let rider = |id: ObjId| id.checked_sub(FOLLOWER_BASE).map(|i| i as usize);
        let is_obstacle = |id: ObjId| id >= OBSTACLE_BASE && id != OBJECT_INVALID;
        let is_rider = |id: ObjId| (FOLLOWER_BASE..OBSTACLE_BASE).contains(&id);
        let yes = |v: bool| Some(Value::Int(v as i32));
        match name {
            "SWMG_GetPlayer" => Some(Value::Object(pc)),
            "SWMG_GetPosition" => {
                let id = object(0);
                Some(Value::Vector(if is_rider(id) {
                    rider(id)
                        .and_then(|i| game.riders.get(i))
                        .map_or([0.0; 3], |r| r.at)
                } else {
                    game.position()
                }))
            }
            "SWMG_PlayAnimation" => {
                // A follower's own clips are for whoever draws it. `boost` and the rest are the bike's.
                let id = object(0);
                if !is_rider(id) && !is_obstacle(id) {
                    game.play(&text(1), int(2) != 0, int(3) != 0, int(4) != 0);
                }
                None
            }
            "SWMG_RemoveAnimation" => {
                let key = text(1).to_ascii_lowercase();
                game.animations.retain(|a| a.name != key);
                None
            }
            "SWMG_GetPlayerSpeed" => Some(Value::Float(game.speed)),
            "SWMG_SetPlayerSpeed" => {
                // A bump takes it no lower than the gear allows, and the start of a gear lifts nothing: the video's meter
                // rises from empty.
                let wanted = float(0);
                game.speed = if game.speed >= game.min_speed {
                    wanted.max(game.min_speed)
                } else {
                    wanted
                };
                None
            }
            "SWMG_GetPlayerMinSpeed" => Some(Value::Float(game.min_speed)),
            "SWMG_SetPlayerMinSpeed" => {
                game.min_speed = float(0);
                None
            }
            "SWMG_GetPlayerMaxSpeed" => Some(Value::Float(game.max_speed)),
            "SWMG_SetPlayerMaxSpeed" => {
                game.max_speed = float(0);
                None
            }
            "SWMG_GetPlayerAccelerationPerSecond" => Some(Value::Float(game.accel)),
            "SWMG_SetPlayerAccelerationPerSecond" => {
                game.accel = float(0);
                None
            }
            "SWMG_GetLateralAccelerationPerSecond" => Some(Value::Float(game.lateral_accel)),
            "SWMG_SetLateralAccelerationPerSecond" => {
                game.lateral_accel = float(0);
                None
            }
            "SWMG_GetPlayerTunnelPos" => Some(Value::Vector(game.tunnel_pos)),
            "SWMG_SetPlayerTunnelPos" => {
                game.tunnel_pos = vector(0);
                None
            }
            "SWMG_GetPlayerTunnelNeg" => Some(Value::Vector(game.tunnel_neg)),
            "SWMG_SetPlayerTunnelNeg" => {
                game.tunnel_neg = vector(0);
                None
            }
            "SWMG_GetPlayerOffset" => Some(Value::Vector(game.offset)),
            "SWMG_SetPlayerOffset" => {
                game.offset = vector(0);
                None
            }
            "SWMG_GetPlayerOrigin" => Some(Value::Vector(game.origin)),
            "SWMG_SetPlayerOrigin" => {
                game.origin = vector(0);
                None
            }
            "SWMG_GetPlayerInvincibility" => Some(Value::Float(game.invincibility)),
            "SWMG_SetPlayerInvincibility" => {
                game.invincibility = float(0);
                None
            }
            "SWMG_GetIsInvulnerable" => yes(game.invulnerable()),
            "SWMG_StartInvulnerability" => {
                game.invulnerable_until = game.time + game.file.player.invincible_for;
                None
            }
            "SWMG_SetSpeedBlurEffect" => {
                game.blur = (int(0) != 0).then(|| float(1));
                None
            }
            "SWMG_GetLastFollowerHit" => Some(Value::Object(
                game.last_rider_hit
                    .map_or(OBJECT_INVALID, |i| FOLLOWER_BASE + i as ObjId),
            )),
            "SWMG_GetLastObstacleHit" => Some(Value::Object(
                game.last_obstacle_hit
                    .map_or(OBJECT_INVALID, |i| OBSTACLE_BASE + i as ObjId),
            )),
            "SWMG_AdjustFollowerHitPoints" | "SWMG_SetFollowerHitPoints" => {
                let time = game.time;
                let set = name == "SWMG_SetFollowerHitPoints" || int(2) != 0;
                let id = object(0);
                let change = int(1) as i64;
                if let Some(r) = rider(id)
                    .filter(|_| is_rider(id))
                    .and_then(|i| game.riders.get_mut(i))
                {
                    r.hit_points = if set { change } else { r.hit_points + change };
                    if r.hit_points <= 0 && r.fell_at.is_none() {
                        r.fell_at = Some(time);
                    }
                    return Some(Value::Int(r.hit_points as i32));
                }
                Some(Value::Int(0))
            }
            "SWMG_GetHitPoints" | "SWMG_GetMaxHitPoints" => {
                let id = object(0);
                let of = rider(id)
                    .filter(|_| is_rider(id))
                    .and_then(|i| game.riders.get(i));
                let (now, most) = of.map_or(
                    (game.file.player.hit_points, game.file.player.max_hit_points),
                    |r| (r.hit_points, r.max_hit_points),
                );
                Some(Value::Int(if name == "SWMG_GetHitPoints" {
                    now as i32
                } else {
                    most as i32
                }))
            }
            "SWMG_GetEnemyCount" => Some(Value::Int(game.riders.len() as i32)),
            "SWMG_GetEnemy" => Some(Value::Object(FOLLOWER_BASE + int(0).max(0) as ObjId)),
            "SWMG_GetObstacleCount" => Some(Value::Int(game.obstacles.len() as i32)),
            "SWMG_GetObstacle" => Some(Value::Object(OBSTACLE_BASE + int(0).max(0) as ObjId)),
            "SWMG_IsPlayer" => yes(object(0) == pc),
            "SWMG_IsEnemy" | "SWMG_IsFollower" => {
                let id = object(0);
                yes(is_rider(id) || (name == "SWMG_IsFollower" && id == pc))
            }
            "SWMG_IsTrigger" => {
                let id = object(0);
                yes(rider(id)
                    .filter(|_| is_rider(id))
                    .and_then(|i| game.riders.get(i))
                    .is_some_and(|r| r.trigger))
            }
            "SWMG_IsObstacle" => yes(is_obstacle(object(0))),
            "SWMG_GetSphereRadius" => Some(Value::Float(game.file.player.radius)),
            "SWMG_GetNumLoops" => Some(Value::Int(game.file.player.loops as i32)),
            "SWMG_GetCameraNearClip" => Some(Value::Float(game.file.near_clip)),
            "SWMG_GetCameraFarClip" => Some(Value::Float(game.file.far_clip)),
            "SWMG_SetCameraClip" => {
                game.file.near_clip = float(0);
                game.file.far_clip = float(1);
                None
            }
            _ => {
                *self.constant_defaults.entry(name.to_string()).or_insert(0) += 1;
                None
            }
        }
    }

    /// The clock in a race, whose scripts time it: an hour of `Mod_MinPerHour` 2 minutes, which `heartbeat` reckons with.
    pub(crate) fn race_clock(&self, routine: &str) -> Option<i32> {
        self.race.as_ref()?;
        let seconds = self.time as f64;
        let whole = seconds.floor() as i64;
        Some(match routine {
            "GetTimeHour" => (13 + whole / 120).rem_euclid(24) as i32,
            "GetTimeMinute" => (whole / 60).rem_euclid(2) as i32,
            "GetTimeSecond" => whole.rem_euclid(60) as i32,
            _ => ((seconds - seconds.floor()) * 1000.0) as i32,
        })
    }
}
