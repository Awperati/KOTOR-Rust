# KotOR-RS animation add-on for Blender (3.6 and later, tried against the 4.x and 5.x API).
#
# One file. Copy it into Blender's add-ons folder (see docs/BLENDER_ANIMATION.md), switch it on in
# Edit > Preferences > Add-ons, and a "KotOR" tab appears in the 3D view's sidebar (press N).
#
# What it does:
#   1. "Load export": reads a folder written by `kotor-tool blender-export` (the character's glTF and its bones.txt), imports
#      the meshes, and builds a rig whose bones are the game's, with the names and the frames the game has.
#   2. You pose and keyframe the bones with Blender's own tools. The scene runs at 30 frames a second, the game's.
#   3. "Export clip" writes <name>.anim.txt (version 2) that the game plays: bone names are checked first.
#
# The first section has no Blender in it, so that the file's format and its checks can be tried outside Blender.

bl_info = {
    "name": "KotOR-RS animation",
    "author": "Frost Technology",
    "version": (1, 0, 0),
    "blender": (3, 6, 0),
    "location": "3D View > Sidebar (N) > KotOR",
    "description": "Make animations for the KotOR-RS game's characters and write them as .anim.txt files",
    "category": "Animation",
}

import math
import os

FORMAT_VERSION = 2
FPS = 30

# Nodes whose forward and sideways movement the game does itself (their height is played).
ROOT_NODES = ("rootdummy", "cutscenedummy", "pelvis_g")


# ----------------------------------------------------------------------------------------------------------------------
# No Blender below this line until "Blender part".
# ----------------------------------------------------------------------------------------------------------------------


def parse_bones(text):
    """The skeleton of a bones.txt: a list of (name, parent or None, (x, y, z), (w, x, y, z)), parents first."""
    bones = []
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        name, parent, place, turn = line.split("\t")
        bones.append(
            (
                name,
                None if parent == "-" else parent,
                tuple(float(v) for v in place.split()),
                tuple(float(v) for v in turn.split()),
            )
        )
    return bones


def clean_name(name):
    """A clip's name as the game takes it: lower case, letters, digits and _."""
    out = "".join(c if c.isalnum() or c == "_" else "_" for c in name.strip().lower())
    return out or "clip"


def format_clip(name, fps, looping, tracks):
    """The text of an .anim.txt. `tracks` maps a bone's name to a list of (frame, (x, y, z), (w, x, y, z)); frames count from 0."""
    lines = [
        "version = %d" % FORMAT_VERSION,
        "name = %s" % clean_name(name),
        "fps = %g" % fps,
        "loop = %s" % ("yes" if looping else "no"),
    ]
    for bone in sorted(tracks):
        for frame, (px, py, pz), (qw, qx, qy, qz) in tracks[bone]:
            lines.append(
                "key %s %g %.6g %.6g %.6g %.6g %.6g %.6g %.6g" % (bone, frame, px, py, pz, qw, qx, qy, qz)
            )
    return "\n".join(lines) + "\n"


def validate_clip(tracks, rig_bones, frame_count, fps):
    """Check a clip before it is written. Returns (errors, warnings): errors stop the export, warnings are for reading.

    `tracks` as in format_clip; `rig_bones` the names of the game's bones.
    """
    errors, warnings = [], []
    known = set(rig_bones)
    extra = sorted(b for b in tracks if b not in known)
    if extra:
        errors.append("bones the game does not have (they cannot be played): " + ", ".join(extra))
    if not tracks:
        errors.append("nothing moves: key at least one bone")
    if frame_count < 2:
        errors.append("the clip is %d frame long; set the end frame after the start" % frame_count)
    if abs(fps - FPS) > 1e-3:
        warnings.append("the scene runs at %g frames a second, the game's is %d: the clip is played at %g" % (fps, FPS, fps))
    still = sorted(b for b in rig_bones if b not in tracks and b not in extra)
    if still:
        warnings.append("%d bones are not keyed and stay at the body's rest (%s)" % (len(still), ", ".join(still[:6]) + (", ..." if len(still) > 6 else "")))
    for bone, keys in tracks.items():
        for frame, _, q in keys:
            n = math.sqrt(sum(v * v for v in q))
            if abs(n - 1.0) > 1e-3:
                errors.append("%s frame %g: the turn is not a unit quaternion (%.4f)" % (bone, frame, n))
                break
    return errors, warnings


# --- small quaternion / matrix helpers on plain tuples, for the world places of the rest pose --------------------------


def q_mul(a, b):
    aw, ax, ay, az = a
    bw, bx, by, bz = b
    return (
        aw * bw - ax * bx - ay * by - az * bz,
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
    )


def q_rotate(q, v):
    w, x, y, z = q
    # v' = q v q^-1
    qv = q_mul(q_mul((w, x, y, z), (0.0, v[0], v[1], v[2])), (w, -x, -y, -z))
    return (qv[1], qv[2], qv[3])


def rest_world(bones):
    """Each bone's rest place and turn in the body's own space: {name: ((x, y, z), (w, x, y, z))}."""
    world = {}
    for name, parent, place, turn in bones:
        if parent is None or parent not in world:
            world[name] = (place, turn)
        else:
            pp, pq = world[parent]
            rotated = q_rotate(pq, place)
            world[name] = ((pp[0] + rotated[0], pp[1] + rotated[1], pp[2] + rotated[2]), q_mul(pq, turn))
    return world


# ----------------------------------------------------------------------------------------------------------------------
# Blender part
# ----------------------------------------------------------------------------------------------------------------------

try:
    import bpy
    from bpy.props import BoolProperty, StringProperty
    from mathutils import Matrix, Quaternion, Vector
except ImportError:  # outside Blender: the section above is all there is
    bpy = None

RIG_NAME = "KotOR Rig"
BONES_KEY = "kotor_bones"  # the bones.txt text, kept on the rig so that a saved .blend carries it


def default_anim_folder():
    base = os.environ.get("LOCALAPPDATA") or os.path.expanduser("~")
    return os.path.join(base, "kotor-rs", "anims")


if bpy is not None:

    def find_rig(context):
        obj = context.active_object
        if obj is not None and obj.type == "ARMATURE" and BONES_KEY in obj:
            return obj
        for o in bpy.data.objects:
            if o.type == "ARMATURE" and BONES_KEY in o:
                return o
        return None

    def rig_bones(rig):
        return parse_bones(rig[BONES_KEY])

    class KOTOR_OT_load_export(bpy.types.Operator):
        """Load a folder written by kotor-tool blender-export: the meshes, and a rig with the game's bones"""

        bl_idname = "kotor.load_export"
        bl_label = "Load export"
        filepath: StringProperty(subtype="FILE_PATH")
        filter_glob: StringProperty(default="*.gltf", options={"HIDDEN"})

        def invoke(self, context, event):
            context.window_manager.fileselect_add(self)
            return {"RUNNING_MODAL"}

        def execute(self, context):
            gltf = self.filepath
            folder = os.path.dirname(gltf)
            bones_file = os.path.join(folder, "bones.txt")
            if not os.path.isfile(gltf) or not os.path.isfile(bones_file):
                self.report({"ERROR"}, "Pick the .gltf that kotor-tool blender-export wrote; bones.txt must be beside it")
                return {"CANCELLED"}
            with open(bones_file, encoding="utf-8") as f:
                text = f.read()
            bones = parse_bones(text)
            before = set(bpy.data.objects)
            bpy.ops.import_scene.gltf(filepath=gltf)
            imported = [o for o in bpy.data.objects if o not in before]
            rig = self.build_rig(context, bones, text)
            # The importer's own armatures are replaced by the game's: meshes follow the new one, through their vertex groups
            # (named for the game's bones, as the importer names a joint's group).
            for o in imported:
                if o.type == "MESH":
                    for m in o.modifiers:
                        if m.type == "ARMATURE":
                            m.object = rig
                    if o.parent is not None and o.parent.type == "ARMATURE":
                        o.parent = rig
                        o.matrix_parent_inverse = Matrix.Identity(4)
            for o in imported:
                if o.type == "ARMATURE":
                    bpy.data.objects.remove(o, do_unlink=True)
            scene = context.scene
            scene.render.fps = FPS
            scene.render.fps_base = 1.0
            context.view_layer.objects.active = rig
            self.report({"INFO"}, "Loaded %d bones. Pose in Pose Mode, key with I, then Export clip." % len(bones))
            return {"FINISHED"}

        def build_rig(self, context, bones, text):
            world = rest_world(bones)
            arm = bpy.data.armatures.new(RIG_NAME)
            rig = bpy.data.objects.new(RIG_NAME, arm)
            context.collection.objects.link(rig)
            rig[BONES_KEY] = text
            context.view_layer.objects.active = rig
            bpy.ops.object.mode_set(mode="EDIT")
            made = {}
            for name, parent, _place, _turn in bones:
                eb = arm.edit_bones.new(name)
                (px, py, pz), (qw, qx, qy, qz) = world[name]
                m = Quaternion((qw, qx, qy, qz)).to_matrix().to_4x4()
                m.translation = Vector((px, py, pz))
                eb.length = 0.06
                eb.matrix = m  # the bone's own frame is the game's node frame, roll and all
                made[name] = eb
            for name, parent, _place, _turn in bones:
                if parent is not None and parent in made:
                    made[name].parent = made[parent]
            bpy.ops.object.mode_set(mode="OBJECT")
            for pb in rig.pose.bones:
                pb.rotation_mode = "QUATERNION"
            arm.display_type = "STICK"
            rig.show_in_front = True
            return rig

    class KOTOR_OT_set_rate(bpy.types.Operator):
        """Run the scene at the game's 30 frames a second"""

        bl_idname = "kotor.set_rate"
        bl_label = "Set 30 fps"

        def execute(self, context):
            context.scene.render.fps = FPS
            context.scene.render.fps_base = 1.0
            return {"FINISHED"}

    def baked_tracks(context, rig, bones):
        """Pose the rig at each frame of the scene's range and read each moved bone's place and turn as the game has them."""
        scene = context.scene
        start, end = scene.frame_start, scene.frame_end
        rest = {n: (Vector(p), Quaternion(q)) for n, _par, p, q in bones}
        samples = {n: [] for n in rest}
        warnings = []
        for f in range(start, end + 1):
            scene.frame_set(f)
            for pb in rig.pose.bones:
                if pb.name not in rest:
                    continue
                local = Matrix.Translation(rest[pb.name][0]) @ rest[pb.name][1].to_matrix().to_4x4() @ pb.matrix_basis
                loc, rot, scl = local.decompose()
                if max(abs(scl[0] - 1), abs(scl[1] - 1), abs(scl[2] - 1)) > 1e-3 and not warnings:
                    warnings.append("a bone is scaled; the game ignores scale")
                samples[pb.name].append((f - start, tuple(loc), (rot.w, rot.x, rot.y, rot.z)))
        scene.frame_set(start)
        tracks = {}
        for name, keys in samples.items():
            rp, rq = rest[name]
            moved = any(
                (Vector(k[1]) - rp).length > 1e-5 or abs(Quaternion(k[2]).dot(rq)) < 1 - 1e-6 for k in keys
            )
            if moved:
                # Keep a turn and its negative apart: the same turn, the shorter way round, frame to frame.
                fixed, last = [], None
                for frame, loc, q in keys:
                    if last is not None and sum(a * b for a, b in zip(last, q)) < 0:
                        q = tuple(-v for v in q)
                    fixed.append((frame, loc, q))
                    last = q
                tracks[name] = fixed
        return tracks, warnings

    class KOTOR_OT_validate(bpy.types.Operator):
        """Check the clip for the game: bone names, the range, the turns"""

        bl_idname = "kotor.validate"
        bl_label = "Validate for KotOR-RS"

        def execute(self, context):
            rig = find_rig(context)
            if rig is None:
                self.report({"ERROR"}, "Load an export first")
                return {"CANCELLED"}
            bones = rig_bones(rig)
            tracks, warnings = baked_tracks(context, rig, bones)
            errors, more = validate_clip(
                tracks,
                [b[0] for b in bones],
                context.scene.frame_end - context.scene.frame_start + 1,
                context.scene.render.fps / context.scene.render.fps_base,
            )
            # A bone of the rig that is not one of the game's: someone added a bone.
            names = {b[0] for b in bones}
            extra = sorted(pb.name for pb in rig.pose.bones if pb.name not in names)
            if extra:
                errors.append("bones the game does not have (add none): " + ", ".join(extra))
            context.scene["kotor_report"] = "\n".join(["ERROR " + e for e in errors] + ["note " + w for w in warnings + more])
            if errors:
                self.report({"ERROR"}, "%d problem(s); see the KotOR panel" % len(errors))
                return {"CANCELLED"}
            self.report({"INFO"}, "Good for the game: %d bones move" % len(tracks))
            return {"FINISHED"}

    class KOTOR_OT_export_clip(bpy.types.Operator):
        """Write the clip as <name>.anim.txt for the game"""

        bl_idname = "kotor.export_clip"
        bl_label = "Export clip"

        def execute(self, context):
            result = bpy.ops.kotor.validate()
            if result != {"FINISHED"}:
                return {"CANCELLED"}
            rig = find_rig(context)
            bones = rig_bones(rig)
            tracks, _ = baked_tracks(context, rig, bones)
            scene = context.scene
            name = clean_name(scene.kotor_clip_name)
            folder = bpy.path.abspath(scene.kotor_folder) or default_anim_folder()
            os.makedirs(folder, exist_ok=True)
            text = format_clip(name, scene.render.fps / scene.render.fps_base, scene.kotor_loop, tracks)
            path = os.path.join(folder, name + ".anim.txt")
            with open(path, "w", encoding="utf-8", newline="\n") as f:
                f.write(text)
            self.report({"INFO"}, "Wrote %s. In the game press F8, then F9 to play it." % path)
            return {"FINISHED"}

    class KOTOR_PT_panel(bpy.types.Panel):
        bl_label = "KotOR-RS animation"
        bl_idname = "KOTOR_PT_panel"
        bl_space_type = "VIEW_3D"
        bl_region_type = "UI"
        bl_category = "KotOR"

        def draw(self, context):
            col = self.layout.column()
            scene = context.scene
            col.operator("kotor.load_export", icon="IMPORT")
            rig = find_rig(context)
            if rig is None:
                col.label(text="Load a folder from kotor-tool blender-export")
                return
            col.label(text="Rig: %d game bones" % len(rig_bones(rig)))
            fps = scene.render.fps / scene.render.fps_base
            row = col.row()
            row.label(text="Scene: %g fps (game: %d)" % (fps, FPS))
            if abs(fps - FPS) > 1e-3:
                row.operator("kotor.set_rate")
            col.prop(scene, "frame_start")
            col.prop(scene, "frame_end")
            col.operator("screen.animation_play", text="Preview", icon="PLAY")
            col.separator()
            col.prop(scene, "kotor_clip_name", text="Clip name")
            col.prop(scene, "kotor_loop", text="Loops")
            col.prop(scene, "kotor_folder", text="Folder")
            col.operator("kotor.validate", icon="CHECKMARK")
            col.operator("kotor.export_clip", icon="EXPORT")
            report = scene.get("kotor_report", "")
            for line in report.splitlines():
                col.label(text=line[:90], icon="ERROR" if line.startswith("ERROR") else "INFO")

    CLASSES = (KOTOR_OT_load_export, KOTOR_OT_set_rate, KOTOR_OT_validate, KOTOR_OT_export_clip, KOTOR_PT_panel)

    def register():
        for c in CLASSES:
            bpy.utils.register_class(c)
        bpy.types.Scene.kotor_clip_name = StringProperty(name="Clip name", default="wave")
        bpy.types.Scene.kotor_loop = BoolProperty(name="Loops", default=True)
        bpy.types.Scene.kotor_folder = StringProperty(
            name="Folder", subtype="DIR_PATH", default=default_anim_folder()
        )

    def unregister():
        for c in reversed(CLASSES):
            bpy.utils.unregister_class(c)
        del bpy.types.Scene.kotor_clip_name
        del bpy.types.Scene.kotor_loop
        del bpy.types.Scene.kotor_folder

    if __name__ == "__main__":
        register()
