//! The Ebon Hawk's turret: what an area's `MiniGame` struct says, played. The player sits on a track of their own and turns.
//! Enemies fly along theirs, fire at the player and are fired at. Scripts hear of it through the `SWMG_` routines.
//!
//! **Read from the install:** every number and name, from the struct of `m12ab` and its models. `nwscript.nss` says the player's
//! offset is a rotation here. **Assumed, not measured:** the offset is degrees, with x up and down, and z round; a turret's
//! forward is +y; a shot flies from its gun to where the crosshair points; a heartbeat every 0.1 s, so that the radar turns; a
//! bank's spreads are the whole width of a cone along a fighter's forward; a shot hits when it comes inside a follower's sphere.
//! **Not built:** the swoop race, `Type` 1. Obstacles do nothing here: nothing in `m12ab` moves into them.

use crate::world::{Kind, ObjId, World};
use kotor_core::ResType;
use kotor_gff::{Gff, Struct, Value};
use kotor_model::Model;
use std::collections::BTreeMap;
use std::rc::Rc;

/// The value of `Type` that means this game.
const TURRET: i64 = 2;
const HEARTBEAT: f32 = 0.1;
const MODULE_HEARTBEAT: f32 = 6.0;
const TRACK_ANIMATION: &str = "track";
const TRACK_NODE: &str = "modelhook";
const CAMERA_NODE: &str = "camerahook";
const AIM_NODE: &str = "target";
const BULLET_TO_PLAYER: i32 = 1;
const DIE: &str = "die";
const DAMAGE: &str = "damage";
const GUN_NODE: &str = "gunbank";
/// How far along its gun a shot starts, from `bullethook0` of `mgg_turret`.
const GUN_LENGTH: f32 = 2.09;

type Mat = [[f32; 4]; 4];

fn sub<'a>(s: &'a Struct, name: &str) -> Option<&'a Struct> {
    match s.get(name) {
        Some(Value::Struct(v)) => Some(v),
        _ => None,
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn minus(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn times(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    times(a, 1.0 / dot(a, a).sqrt().max(1e-6))
}

/// How close a line from `from` to `to` comes to `point`.
fn nearest(from: [f32; 3], to: [f32; 3], point: [f32; 3]) -> f32 {
    let along = minus(to, from);
    let k = (dot(minus(point, from), along) / dot(along, along).max(1e-6)).clamp(0.0, 1.0);
    let off = minus(point, add(from, times(along, k)));
    dot(off, off).sqrt()
}

/// One of a follower's guns.
#[derive(Clone, Debug, Default)]
pub struct GunBank {
    pub gun_model: String,
    pub fire_sound: String,
    pub bullet_model: String,
    pub hit_sound: String,
    pub damage: i32,
    pub lifespan: f32,
    pub rate: f32,
    pub speed: f32,
    pub target_type: i32,
    pub inaccuracy: f32,
    pub sensing: f32,
    pub horizontal: f32,
    pub vertical: f32,
    wait: f32,
}

/// An animation playing on a follower, on whichever of its models has it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayedAnimation {
    pub name: String,
    pub time: f32,
    pub looping: bool,
    pub overlay: bool,
}

/// The player, or an enemy.
#[derive(Clone, Debug, Default)]
pub struct Follower {
    pub id: ObjId,
    pub track: String,
    /// Each model, and whether it turns with the player.
    pub models: Vec<(String, bool)>,
    pub hit_points: i32,
    pub max_hit_points: i32,
    pub sphere: f32,
    pub death_sound: String,
    scripts: BTreeMap<String, String>,
    pub guns: Vec<GunBank>,
    /// How far along the track it is.
    time: f32,
    length: f32,
    pub at: [f32; 3],
    /// Its track's orientation here: its rows, as a matrix's.
    pub placed: Mat,
    pub animations: Vec<PlayedAnimation>,
    /// Seconds left of its death, and whether it is gone.
    pub dying: Option<f32>,
    pub gone: bool,
    heartbeat: f32,
}

/// A shot in the air.
#[derive(Clone, Debug)]
pub struct Bullet {
    pub model: String,
    pub at: [f32; 3],
    pub moving: [f32; 3],
    life: f32,
    damage: i32,
    to_player: bool,
    hit_sound: String,
    shooter: ObjId,
}

/// The game going on.
pub struct MiniGame {
    pub near: f32,
    pub far: f32,
    pub view_angle: f32,
    pub music: String,
    pub player: Follower,
    pub enemies: Vec<Follower>,
    pub bullets: Vec<Bullet>,
    /// The player's turn in degrees: up and down, then round.
    pub offset: [f32; 3],
    /// Holding the fire button.
    pub firing: bool,
    /// The fire button has gone down since the guns last looked: a tap fires too.
    pub fire_once: bool,
    limit_low: [f32; 3],
    limit_high: [f32; 3],
    pub turn_per_second: f32,
    camera_at: [f32; 3],
    aim_at: [f32; 3],
    tracks: BTreeMap<String, Rc<Model>>,
    guns_at: Vec<[f32; 3]>,
    /// Sounds and things to happen, for what draws the game to take.
    pub sounds: Vec<String>,
    /// Followers that have just started to die.
    pub deaths: Vec<ObjId>,
    pub last_hit_shooter: ObjId,
    pub last_hit_damage: i32,
    pub last_hp_change: i32,
    module_heartbeat: f32,
}

fn follower_of(s: &Struct) -> Follower {
    let word = |name: &str| s.str(name).unwrap_or("").to_string();
    let f = |name: &str| s.float(name).unwrap_or(0.0) as f32;
    let mut out = Follower {
        track: word("Track").to_ascii_lowercase(),
        hit_points: s.int("Hit_Points").unwrap_or(0) as i32,
        max_hit_points: s.int("Max_HPs").unwrap_or(0) as i32,
        sphere: f("Sphere_Radius"),
        placed: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        ..Follower::default()
    };
    if let Some(sounds) = sub(s, "Sounds") {
        out.death_sound = sounds.str("Death").unwrap_or("").to_string();
    }
    for m in s.list("Models").unwrap_or_default() {
        out.models.push((
            m.str("Model").unwrap_or("").to_ascii_lowercase(),
            m.int("RotatingModel").unwrap_or(0) != 0,
        ));
    }
    if let Some(scripts) = sub(s, "Scripts") {
        for name in ["OnCreate", "OnHeartbeat", "OnDamage", "OnDeath"] {
            let script = scripts.str(name).unwrap_or("");
            if !script.is_empty() {
                out.scripts
                    .insert(name.to_string(), script.to_ascii_lowercase());
            }
        }
    }
    for g in s.list("Gun_Banks").unwrap_or_default() {
        let number = |name: &str| g.float(name).unwrap_or(0.0) as f32;
        let mut bank = GunBank {
            gun_model: g.str("Gun_Model").unwrap_or("").to_ascii_lowercase(),
            fire_sound: g.str("Fire_Sound").unwrap_or("").to_string(),
            inaccuracy: number("Inaccuracy"),
            sensing: number("Sensing_Radius"),
            horizontal: number("Horiz_Spread"),
            vertical: number("Vert_Spread"),
            ..GunBank::default()
        };
        if let Some(b) = sub(g, "Bullet") {
            bank.bullet_model = b.str("Bullet_Model").unwrap_or("").to_ascii_lowercase();
            bank.hit_sound = b.str("Collision_Sound").unwrap_or("").to_string();
            bank.damage = b.int("Damage").unwrap_or(0) as i32;
            bank.lifespan = b.float("Lifespan").unwrap_or(0.0) as f32;
            bank.rate = b.float("Rate_Of_Fire").unwrap_or(0.0) as f32;
            bank.speed = b.float("Speed").unwrap_or(0.0) as f32;
            bank.target_type = b.int("Target_Type").unwrap_or(0) as i32;
        }
        out.guns.push(bank);
    }
    out
}

impl MiniGame {
    /// The player's turn, as a matrix's rows without the place.
    fn turn(&self) -> [[f32; 3]; 3] {
        let (sp, cp) = self.offset[0].to_radians().sin_cos();
        let (sy, cy) = self.offset[2].to_radians().sin_cos();
        [
            [cy, -sy * cp, sy * sp],
            [sy, cy * cp, -cy * sp],
            [0.0, sp, cp],
        ]
    }

    fn turned(&self, v: [f32; 3]) -> [f32; 3] {
        let r = self.turn();
        [dot(r[0], v), dot(r[1], v), dot(r[2], v)]
    }

    /// What a model that turns with the player has as its matrix.
    pub fn turning_matrix(&self) -> Mat {
        let r = self.turn();
        let p = self.player.at;
        [
            [r[0][0], r[0][1], r[0][2], p[0]],
            [r[1][0], r[1][1], r[1][2], p[1]],
            [r[2][0], r[2][1], r[2][2], p[2]],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    /// Where the player's eye is, and a point it looks at, through the crosshair.
    pub fn camera(&self) -> ([f32; 3], [f32; 3]) {
        let eye = add(self.player.at, self.turned(self.camera_at));
        let looks = self.turned(minus(self.aim_at, self.camera_at));
        (eye, add(eye, looks))
    }

    /// Turn by degrees, inside what the area allows.
    pub fn turn_by(&mut self, up: f32, round: f32) {
        self.offset[0] = (self.offset[0] + up).clamp(self.limit_low[0], self.limit_high[0]);
        self.offset[2] = (self.offset[2] + round).clamp(self.limit_low[2], self.limit_high[2]);
    }

    /// Where a gun of the player's sits, before the turn.
    pub fn gun_place(&self, bank: usize) -> [f32; 3] {
        self.guns_at.get(bank).copied().unwrap_or([0.0; 3])
    }

    /// Point the crosshair at something, as near as the turret goes.
    pub fn point_at(&mut self, at: [f32; 3]) {
        let (eye, looks) = self.camera();
        let want = unit(minus(at, eye));
        let now = unit(minus(looks, eye));
        let up = want[2].asin().to_degrees() - now[2].asin().to_degrees();
        let round = want[0].atan2(want[1]).to_degrees() - now[0].atan2(now[1]).to_degrees();
        // Rounds the short way: `atan2` measures from +y, and a turn of +z goes to -x.
        let round = (-round + 540.0).rem_euclid(360.0) - 180.0;
        self.turn_by(up, round);
    }

    pub fn follower_mut(&mut self, id: ObjId) -> Option<&mut Follower> {
        if self.player.id == id {
            return Some(&mut self.player);
        }
        self.enemies.iter_mut().find(|e| e.id == id)
    }

    pub fn follower(&self, id: ObjId) -> Option<&Follower> {
        if self.player.id == id {
            return Some(&self.player);
        }
        self.enemies.iter().find(|e| e.id == id)
    }

    /// How many enemies are still flying.
    pub fn alive(&self) -> usize {
        self.enemies
            .iter()
            .filter(|e| e.dying.is_none() && !e.gone)
            .count()
    }

    fn place(track: Option<&Rc<Model>>, f: &mut Follower) {
        let Some(track) = track else { return };
        let pose = track
            .animation(TRACK_ANIMATION)
            .map(|a| a.sample(track, f.time));
        let found = track
            .world_matrices(pose.as_ref())
            .into_iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(TRACK_NODE));
        if let Some((_, m)) = found {
            f.at = [m[0][3], m[1][3], m[2][3]];
            f.placed = m;
        }
    }

    fn fire_player(&mut self, dt: f32) {
        let (eye, looks) = self.camera();
        let far = add(eye, times(minus(looks, eye), 1000.0));
        let forward = unit(minus(looks, eye));
        let from = self.player.at;
        let places: Vec<[f32; 3]> = self.guns_at.iter().map(|g| self.turned(*g)).collect();
        let firing = self.firing || std::mem::take(&mut self.fire_once);
        let shooter = self.player.id;
        for (n, bank) in self.player.guns.iter_mut().enumerate() {
            bank.wait -= dt;
            if !firing || bank.wait > 0.0 {
                continue;
            }
            bank.wait = bank.rate;
            let at = add(
                add(from, places.get(n).copied().unwrap_or([0.0; 3])),
                times(forward, GUN_LENGTH),
            );
            self.bullets.push(Bullet {
                model: bank.bullet_model.clone(),
                at,
                moving: times(unit(minus(far, at)), bank.speed),
                life: bank.lifespan,
                damage: bank.damage,
                to_player: bank.target_type == BULLET_TO_PLAYER,
                hit_sound: bank.hit_sound.clone(),
                shooter,
            });
            if !bank.fire_sound.is_empty() {
                self.sounds.push(bank.fire_sound.clone());
            }
        }
    }

    /// Whether a fighter with this placement can see the player for this bank.
    fn in_sight(placed: &Mat, at: [f32; 3], bank: &GunBank, player: [f32; 3]) -> bool {
        let to = minus(player, at);
        let far = dot(to, to).sqrt();
        if far > bank.sensing {
            return false;
        }
        let right = [placed[0][0], placed[1][0], placed[2][0]];
        let ahead = [placed[0][1], placed[1][1], placed[2][1]];
        let up = [placed[0][2], placed[1][2], placed[2][2]];
        let forward = dot(to, ahead);
        forward > 0.0
            && dot(to, right).atan2(forward).to_degrees().abs() <= bank.horizontal / 2.0
            && dot(to, up).atan2(forward).to_degrees().abs() <= bank.vertical / 2.0
    }

    /// Everything that moves. Returns who was hit, by how much.
    fn advance(
        &mut self,
        dt: f32,
        roll: &mut dyn FnMut() -> f32,
    ) -> Vec<(ObjId, i32, ObjId, String)> {
        let tracks = std::mem::take(&mut self.tracks);
        Self::place(tracks.get(&self.player.track), &mut self.player);
        let player = self.player.at;
        for e in &mut self.enemies {
            if e.gone {
                continue;
            }
            if let Some(left) = e.dying.as_mut() {
                *left -= dt;
                if *left <= 0.0 {
                    e.gone = true;
                }
                continue;
            }
            e.time += dt;
            if e.length > 0.0 {
                e.time %= e.length;
            }
            Self::place(tracks.get(&e.track), e);
            let (at, placed, shooter) = (e.at, e.placed, e.id);
            for bank in &mut e.guns {
                bank.wait -= dt;
                if bank.wait > 0.0 || !Self::in_sight(&placed, at, bank, player) {
                    continue;
                }
                bank.wait = bank.rate;
                let off = bank.inaccuracy;
                let way = unit(minus(player, at));
                let way = unit(add(
                    way,
                    [
                        (roll() - 0.5) * off,
                        (roll() - 0.5) * off,
                        (roll() - 0.5) * off,
                    ],
                ));
                self.bullets.push(Bullet {
                    model: bank.bullet_model.clone(),
                    at,
                    moving: times(way, bank.speed),
                    life: bank.lifespan,
                    damage: bank.damage,
                    to_player: bank.target_type == BULLET_TO_PLAYER,
                    hit_sound: bank.hit_sound.clone(),
                    shooter,
                });
                if !bank.fire_sound.is_empty() {
                    self.sounds.push(bank.fire_sound.clone());
                }
            }
        }
        self.tracks = tracks;
        self.fire_player(dt);

        // Shots go on, and stop at what they meet.
        let mut hits = Vec::new();
        let player_ball = (self.player.id, self.player.at, self.player.sphere);
        let targets: Vec<(ObjId, [f32; 3], f32)> = self
            .enemies
            .iter()
            .filter(|e| e.dying.is_none() && !e.gone)
            .map(|e| (e.id, e.at, e.sphere))
            .collect();
        self.bullets.retain_mut(|b| {
            let next = add(b.at, times(b.moving, dt));
            b.life -= dt;
            let hit = if b.to_player {
                (nearest(b.at, next, player_ball.1) < player_ball.2).then_some(player_ball.0)
            } else {
                targets
                    .iter()
                    .find(|(_, at, sphere)| nearest(b.at, next, *at) < *sphere)
                    .map(|t| t.0)
            };
            b.at = next;
            if let Some(id) = hit {
                hits.push((id, b.damage, b.shooter, b.hit_sound.clone()));
                return false;
            }
            b.life > 0.0
        });

        // What is playing goes on.
        let advance = |f: &mut Follower| {
            for a in &mut f.animations {
                a.time += dt;
            }
        };
        advance(&mut self.player);
        for e in &mut self.enemies {
            advance(e);
        }
        hits
    }
}

impl World {
    /// The area's minigame, if it has the kind that is built.
    pub(crate) fn start_minigame(&mut self, area: &str) {
        self.minigame = None;
        let Some(are) = self
            .inst
            .read(area, ResType::ARE)
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
        else {
            return;
        };
        let Some(m) = sub(&are.root, "MiniGame") else {
            return;
        };
        if m.int("Type").unwrap_or(0) != TURRET {
            return;
        }
        let Some(p) = sub(m, "Player") else {
            return;
        };
        let f = |s: &Struct, name: &str| s.float(name).unwrap_or(0.0) as f32;
        let mut player = follower_of(p);
        player.id = self.add(Kind::Waypoint, "minigame_player");
        let mut enemies: Vec<Follower> = m
            .list("Enemies")
            .unwrap_or_default()
            .iter()
            .map(follower_of)
            .collect();
        for (n, e) in enemies.iter_mut().enumerate() {
            e.id = self.add(Kind::Waypoint, &format!("minigame_enemy{n}"));
        }
        let mut tracks = BTreeMap::new();
        for name in std::iter::once(&player.track).chain(enemies.iter().map(|e| &e.track)) {
            if let Some(model) = self.minigame_model(name) {
                tracks.insert(name.clone(), model);
            }
        }
        for e in &mut enemies {
            e.length = tracks
                .get(&e.track)
                .and_then(|t: &Rc<Model>| t.animation(TRACK_ANIMATION))
                .map_or(0.0, |a| a.length);
        }
        let node = |model: Option<Rc<Model>>, name: &str| -> [f32; 3] {
            model
                .and_then(|m| {
                    m.world_matrices(None)
                        .into_iter()
                        .find(|(n, _)| n.eq_ignore_ascii_case(name))
                })
                .map_or([0.0; 3], |(_, m)| [m[0][3], m[1][3], m[2][3]])
        };
        let camera_model = self.minigame_model(&p.str("Camera").unwrap_or("").to_ascii_lowercase());
        let turret = player
            .models
            .first()
            .map(|m| m.0.clone())
            .unwrap_or_default();
        let turret_model = self.minigame_model(&turret);
        let guns_at = (0..player.guns.len())
            .map(|n| node(turret_model.clone(), &format!("{GUN_NODE}{n}")))
            .collect();
        let mut game = MiniGame {
            near: f(m, "Near_Clip"),
            far: f(m, "Far_Clip"),
            view_angle: f(m, "CameraViewAngle"),
            music: m.str("Music").unwrap_or("").to_string(),
            offset: [
                f(p, "Start_Offset_X"),
                f(p, "Start_Offset_Y"),
                f(p, "Start_Offset_Z"),
            ],
            limit_low: [f(p, "TunnelXNeg"), f(p, "TunnelYNeg"), f(p, "TunnelZNeg")],
            limit_high: [f(p, "TunnelXPos"), f(p, "TunnelYPos"), f(p, "TunnelZPos")],
            turn_per_second: f(m, "MovementPerSec"),
            camera_at: node(camera_model, CAMERA_NODE),
            aim_at: node(turret_model, AIM_NODE),
            player,
            enemies,
            bullets: Vec::new(),
            firing: false,
            fire_once: false,
            tracks,
            guns_at,
            sounds: Vec::new(),
            deaths: Vec::new(),
            last_hit_shooter: 0,
            last_hit_damage: 0,
            last_hp_change: 0,
            module_heartbeat: MODULE_HEARTBEAT,
        };
        // Where the turret's own limits are the same number each way, it does not turn that way.
        game.turn_by(0.0, 0.0);
        let creators: Vec<(ObjId, Option<String>)> = game
            .enemies
            .iter()
            .chain(std::iter::once(&game.player))
            .map(|e| (e.id, e.scripts.get("OnCreate").cloned()))
            .collect();
        self.minigame = Some(Box::new(game));
        // Nothing else here runs a module's own scripts, and this game's count on them: it counts its fighters from nothing.
        self.run_module_script("Mod_OnModLoad");
        for (id, script) in creators {
            if let Some(script) = script {
                let _ = self.run_script(&script, id);
            }
        }
    }

    fn run_module_script(&mut self, name: &str) {
        let module = self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Module && o.tag.eq_ignore_ascii_case(&self.module))
            .find_map(|o| o.scripts.get(name).map(|s| (o.id, s.clone())));
        if let Some((id, script)) = module {
            let _ = self.run_script(&script, id);
        }
    }

    fn minigame_model(&mut self, name: &str) -> Option<Rc<Model>> {
        let mdl = self.inst.read(name, ResType::MDL).ok()?;
        let mdx = self.inst.read(name, ResType::MDX).unwrap_or_default();
        kotor_model::parse_model(&mdl, &mdx).ok().map(Rc::new)
    }

    /// The game goes on by `dt`.
    pub(crate) fn update_minigame(&mut self, dt: f32) {
        let Some(mut game) = self.minigame.take() else {
            return;
        };
        let mut numbers = Vec::with_capacity(12);
        for _ in 0..12 {
            numbers.push(self.roll(1000) as f32 / 1000.0);
        }
        let mut next = numbers.into_iter().cycle();
        let mut roll = move || next.next().unwrap_or(0.5);
        let hits = game.advance(dt, &mut roll);
        game.player.heartbeat -= dt;
        let heartbeat = if game.player.heartbeat <= 0.0 {
            game.player.heartbeat = HEARTBEAT;
            game.player.scripts.get("OnHeartbeat").cloned()
        } else {
            None
        };
        game.module_heartbeat -= dt;
        let module_beat = game.module_heartbeat <= 0.0;
        if module_beat {
            game.module_heartbeat = MODULE_HEARTBEAT;
        }
        let player = game.player.id;
        self.minigame = Some(game);
        if module_beat {
            self.run_module_script("Mod_OnHeartbeat");
        }
        if let Some(script) = heartbeat {
            let _ = self.run_script(&script, player);
        }
        for (target, damage, shooter, sound) in hits {
            self.minigame_hit(target, damage, shooter, &sound);
        }
    }

    /// A shot has struck.
    fn minigame_hit(&mut self, target: ObjId, damage: i32, shooter: ObjId, sound: &str) {
        let Some(game) = self.minigame.as_mut() else {
            return;
        };
        game.last_hit_shooter = shooter;
        game.last_hit_damage = damage;
        game.last_hp_change = -damage;
        if !sound.is_empty() {
            game.sounds.push(sound.to_string());
        }
        let Some(f) = game.follower_mut(target) else {
            return;
        };
        f.hit_points -= damage;
        let script = f.scripts.get("OnDamage").cloned();
        match script {
            Some(script) => {
                let _ = self.run_script(&script, target);
            }
            None => self.minigame_on_damage(target),
        }
    }

    /// `SWMG_OnDamage`, what happens if a script says nothing else: it shows, and at nothing it dies.
    pub(crate) fn minigame_on_damage(&mut self, id: ObjId) {
        let Some(f) = self.minigame.as_mut().and_then(|g| g.follower_mut(id)) else {
            return;
        };
        let dead = f.hit_points <= 0;
        let script = f.scripts.get("OnDeath").cloned();
        if !dead {
            f.play(DAMAGE, false, false);
            return;
        }
        if f.dying.is_some() {
            return;
        }
        match script {
            Some(script) => {
                let _ = self.run_script(&script, id);
            }
            None => self.minigame_on_death(id),
        }
    }

    /// `SWMG_OnDeath`, what happens if a script says nothing else: its `die`, with its sound.
    pub(crate) fn minigame_on_death(&mut self, id: ObjId) {
        let models: Vec<String> = self
            .minigame
            .as_ref()
            .and_then(|g| g.follower(id))
            .map_or(Vec::new(), |f| {
                f.models.iter().map(|m| m.0.clone()).collect()
            });
        let mut length = 0.0_f32;
        for name in models {
            if let Some(m) = self.minigame_model(&name) {
                length = length.max(m.animation(DIE).map_or(0.0, |a| a.length));
            }
        }
        let Some(game) = self.minigame.as_mut() else {
            return;
        };
        if game.player.id == id {
            return;
        }
        let Some(f) = game.follower_mut(id) else {
            return;
        };
        f.dying = Some(length);
        f.guns.clear();
        f.play(DIE, false, false);
        let sound = f.death_sound.clone();
        game.deaths.push(id);
        if !sound.is_empty() {
            game.sounds.push(sound);
        }
    }
}

impl Follower {
    /// `SWMG_PlayAnimation`. One that is not an overlay takes the place of the last such.
    pub fn play(&mut self, name: &str, looping: bool, overlay: bool) {
        self.animations
            .retain(|a| a.overlay != overlay || (overlay && a.name != name));
        self.animations.push(PlayedAnimation {
            name: name.to_string(),
            time: 0.0,
            looping,
            overlay,
        });
    }
}
