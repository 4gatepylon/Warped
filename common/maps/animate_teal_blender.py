"""`animate_teal_blender.py` runs inside Blender to give the Sketchfab robot "Teal V.2" a clean rig and five clips.

`prepare_models.py` runs it as `blender -b --factory-startup -P animate_teal_blender.py -- <scene.gltf> <folder>`.
It reads the downloaded glTF, makes the T-pose that starts the model's only clip its rest pose (the download's own rest
pose throws the arms metres away), drops that clip and the stray icosphere, keyframes the clips `Idle`, `Walk`, `Run`,
`Jump` and `Death` on the model's own bones, and writes `scene.gltf`, `scene.bin` and `textures/` into `<folder>`.

Interface:
    <scene.gltf>: The downloaded model.
    <folder>: The folder to write the animated model into.

Example:
    $ blender -b --factory-startup -P animate_teal_blender.py -- .build/sketchfab/teal_robot/scene.gltf out/

Must know:
    - Blender's Python has no click, so `animate_teal_blender.py` reads its two paths from after `--`, and only
      `prepare_models.py` calls it.
    - Every angle is in degrees about an axis of the rest pose, in Blender's frame: x is the robot's left, -y its
      front, z up. A positive turn about x swings a hanging limb backward and tips an upright one forward.
    - In the T-pose each forearm points forward, bent 90° at the elbow, so an elbow straightens by turning about z.
    - `Walk` and `Run` stay in place, and `Idle`, `Walk` and `Run` loop; `Jump` and `Death` end on a pose to hold.
"""

import math
import sys
from collections.abc import Callable
from pathlib import Path

import bpy
import mathutils

FRAMES_PER_S: int = 30
STRAY_OBJECT_NAME: str = "Icosphere"
TEXTURE_FOLDER_NAME: str = "textures"
AXES: dict[str, tuple[float, float, float]] = {"x": (1.0, 0.0, 0.0), "y": (0.0, 1.0, 0.0), "z": (0.0, 0.0, 1.0)}

ROOT: str = "Bone_01"
WAIST: str = "Spine_02"
CHEST: str = "Spine.003_05"
HEAD: str = "Head_06"
SHOULDERS: dict[str, str] = {"L": "Arm.L_043", "R": "Arm.R_069"}
ELBOWS: dict[str, str] = {"L": "Arm.L.001_044", "R": "Arm.R.001_070"}
HIPS: dict[str, str] = {"L": "Leg.L.001_096", "R": "Leg.R.001_0104"}
KNEES: dict[str, str] = {"L": "Leg.L.002_097", "R": "Leg.R.002_0105"}
ANKLES: dict[str, str] = {"L": "Foot.L_099", "R": "Foot.R_0107"}
SIDE_SIGNS: dict[str, float] = {"L": 1.0, "R": -1.0}
STRIDE_PHASES: dict[str, float] = {"L": 0.0, "R": 0.5}

ARM_DOWN_DEG: float = 72.0
REST_ELBOW_DEG: float = 90.0
IDLE_S: float = 2.0
IDLE_BREATH_DEG: float = 2.0
IDLE_BOB_M: float = 0.006
IDLE_LOOK_DEG: float = 8.0
IDLE_ELBOW_DEG: float = 12.0
WALK_S: float = 1.0
RUN_S: float = 0.6
JUMP_S: float = 0.5
DEATH_S: float = 1.4

Pose = tuple[dict[str, list[tuple[str, float]]], mathutils.Vector]


def _get_argument_paths() -> tuple[Path, Path]:
    """`_get_argument_paths` reads the source glTF and the output folder from the arguments after `--`.

    Returns:
        The downloaded `scene.gltf` and the folder to write into.

    Example:
        >>> source, folder = _get_argument_paths()
    """
    if "--" not in sys.argv or len(sys.argv) - sys.argv.index("--") != 3:
        raise SystemExit(f"animate_teal_blender.py: expected `-- <scene.gltf> <folder>`, got {sys.argv}")
    first_index: int = sys.argv.index("--") + 1
    return Path(sys.argv[first_index]), Path(sys.argv[first_index + 1])


def _make_clean_rig(source: Path) -> bpy.types.Object:
    """`_make_clean_rig` imports `source` and gives it a rest pose at the T-pose that starts its only clip.

    The meshes keep that pose as their shape, the armature takes it as rest, and every transform above the armature
    (Sketchfab's scales of 100 and 0.01) is applied, so the armature sits at the origin in metres with no clip.

    Args:
        source: The downloaded `scene.gltf`.

    Returns:
        The armature.

    Example:
        >>> armature = _make_clean_rig(Path("scene.gltf"))
    """
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(source))
    scene: bpy.types.Scene = bpy.context.scene
    bpy.data.objects.remove(bpy.data.objects[STRAY_OBJECT_NAME])
    armatures: list[bpy.types.Object] = [o for o in scene.objects if o.type == "ARMATURE"]
    if len(armatures) != 1:
        raise SystemExit(f"_make_clean_rig: expected one armature in {source}, found {len(armatures)}")
    armature: bpy.types.Object = armatures[0]
    meshes: list[bpy.types.Object] = [o for o in scene.objects if o.type == "MESH"]
    scene.frame_set(int(armature.animation_data.action.frame_range[0]))
    for mesh in meshes:
        with bpy.context.temp_override(object=mesh, active_object=mesh, selected_objects=[mesh]):
            bpy.ops.object.modifier_apply(modifier=mesh.modifiers["Armature"].name)
    bpy.context.view_layer.objects.active = armature
    for scene_object in scene.objects:
        scene_object.select_set(scene_object == armature)
    bpy.ops.object.mode_set(mode="POSE")
    bpy.ops.pose.select_all(action="SELECT")
    bpy.ops.pose.armature_apply(selected=False)
    bpy.ops.object.mode_set(mode="OBJECT")
    armature.animation_data_clear()
    for action in list(bpy.data.actions):
        bpy.data.actions.remove(action)
    for mesh in meshes:
        mesh.modifiers.new("Armature", "ARMATURE").object = armature
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    for scene_object in list(scene.objects):
        if scene_object.type == "EMPTY":
            bpy.data.objects.remove(scene_object)
    for scene_object in scene.objects:
        scene_object.select_set(True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    return armature


def _get_wave(t: float, phase: float = 0.0) -> float:
    """`_get_wave` gives sin(2π(t + phase)), a wave with period 1 in t.

    Args:
        t: The time as a fraction of the loop.
        phase: The shift, as a fraction of the loop.

    Returns:
        A value from -1 to 1.

    Example:
        >>> _get_wave(0.25)
        1.0
    """
    return math.sin(2.0 * math.pi * (t + phase))


def _get_ease(t: float) -> float:
    """`_get_ease` eases t from 0 to 1 with a smoothstep, clamping it to that range first.

    Args:
        t: The progress, any number.

    Returns:
        The eased progress, from 0 to 1.

    Example:
        >>> _get_ease(0.5)
        0.5
    """
    clamped: float = min(max(t, 0.0), 1.0)
    return clamped * clamped * (3.0 - 2.0 * clamped)


def _get_elbow_turns(side: str, bend_deg: float) -> list[tuple[str, float]]:
    """`_get_elbow_turns` bends one elbow `bend_deg` forward from straight, from the T-pose's 90°.

    Args:
        side: `L` or `R`.
        bend_deg: How far the forearm bends forward from the upper arm's line.

    Returns:
        The elbow's turns.

    Example:
        >>> _get_elbow_turns("L", 15.0)
        [('z', 75.0)]
    """
    return [("z", SIDE_SIGNS[side] * (REST_ELBOW_DEG - bend_deg))]


def _add_arms_down(turns: dict[str, list[tuple[str, float]]], swing_deg: dict[str, float], elbow_deg: float) -> None:
    """`_add_arms_down` lowers both arms from the T-pose, swings them forward or back, and bends the elbows.

    Args:
        turns: The pose's turns, to add the arms' to.
        swing_deg: Each side's forward swing (`L`, `R`); positive is forward.
        elbow_deg: How far both forearms bend forward.

    Example:
        >>> _add_arms_down(turns, {"L": 10.0, "R": -10.0}, 15.0)
    """
    for side, sign in SIDE_SIGNS.items():
        turns[SHOULDERS[side]] = [("y", sign * ARM_DOWN_DEG), ("x", -swing_deg[side])]
        turns[ELBOWS[side]] = _get_elbow_turns(side, elbow_deg)


def _get_idle_pose(t: float) -> Pose:
    """`_get_idle_pose` is the idle loop at t (0 to 1): arms down, a slow breath, and a glance to each side.

    Args:
        t: The time as a fraction of the loop.

    Returns:
        The bones' turns and the root's offset in metres.

    Example:
        >>> turns, offset = _get_idle_pose(0.0)
    """
    turns: dict[str, list[tuple[str, float]]] = {
        CHEST: [("x", IDLE_BREATH_DEG * _get_wave(t))],
        HEAD: [("z", IDLE_LOOK_DEG * _get_wave(t, 0.1))],
    }
    _add_arms_down(turns, {"L": 0.0, "R": 0.0}, IDLE_ELBOW_DEG + IDLE_BREATH_DEG * _get_wave(t))
    return turns, mathutils.Vector((0.0, 0.0, IDLE_BOB_M * _get_wave(2.0 * t)))


def _make_stride_pose(t: float, gait: dict[str, float]) -> Pose:
    """`_make_stride_pose` is a walk or run loop at t: legs swinging in turn, arms against them, a bob each step.

    Args:
        t: The time as a fraction of the loop (one step of each leg).
        gait: `hip_deg` (swing each way), `knee_deg` (bend while the leg swings forward), `contact_knee_deg` (bend as
            the foot lands), `arm_deg` (arm swing each way), `elbow_deg`, `lean_deg` (forward), `twist_deg` (chest)
            and `bob_m` (the root's rise and fall).

    Returns:
        The bones' turns and the root's offset in metres.

    Example:
        >>> turns, offset = _make_stride_pose(0.25, WALK_GAIT)
    """
    turns: dict[str, list[tuple[str, float]]] = {}
    arm_swing_deg: dict[str, float] = {}
    for side in SIDE_SIGNS:
        phase: float = t + STRIDE_PHASES[side]
        hip_forward_deg: float = gait["hip_deg"] * _get_wave(phase, 0.25)
        swing_frac: float = max(0.0, _get_wave(phase, 0.5))
        knee_deg: float = gait["contact_knee_deg"] + gait["knee_deg"] * swing_frac
        turns[HIPS[side]] = [("x", -hip_forward_deg)]
        turns[KNEES[side]] = [("x", knee_deg)]
        turns[ANKLES[side]] = [("x", -0.5 * knee_deg + 0.4 * hip_forward_deg)]
        arm_swing_deg[side] = -gait["arm_deg"] * _get_wave(phase, 0.25)
    _add_arms_down(turns, arm_swing_deg, gait["elbow_deg"])
    turns[WAIST] = [("x", gait["lean_deg"])]
    turns[CHEST] = [("z", gait["twist_deg"] * _get_wave(t, 0.25))]
    turns[HEAD] = [("x", -0.6 * gait["lean_deg"]), ("z", -0.5 * gait["twist_deg"] * _get_wave(t, 0.25))]
    bob_m: float = gait["bob_m"] * abs(_get_wave(t, 0.25)) - gait["bob_m"]
    return turns, mathutils.Vector((0.0, 0.0, bob_m))


WALK_GAIT: dict[str, float] = {
    "hip_deg": 24.0,
    "knee_deg": 42.0,
    "contact_knee_deg": 6.0,
    "arm_deg": 20.0,
    "elbow_deg": 18.0,
    "lean_deg": 3.0,
    "twist_deg": 5.0,
    "bob_m": 0.025,
}
RUN_GAIT: dict[str, float] = {
    "hip_deg": 38.0,
    "knee_deg": 85.0,
    "contact_knee_deg": 14.0,
    "arm_deg": 38.0,
    "elbow_deg": 75.0,
    "lean_deg": 12.0,
    "twist_deg": 9.0,
    "bob_m": 0.05,
}


def _get_walk_pose(t: float) -> Pose:
    """`_get_walk_pose` is the walk loop at t.

    Args:
        t: The time as a fraction of the loop.

    Returns:
        The bones' turns and the root's offset in metres.

    Example:
        >>> turns, offset = _get_walk_pose(0.5)
    """
    return _make_stride_pose(t, WALK_GAIT)


def _get_run_pose(t: float) -> Pose:
    """`_get_run_pose` is the run loop at t.

    Args:
        t: The time as a fraction of the loop.

    Returns:
        The bones' turns and the root's offset in metres.

    Example:
        >>> turns, offset = _get_run_pose(0.5)
    """
    return _make_stride_pose(t, RUN_GAIT)


def _get_jump_pose(t: float) -> Pose:
    """`_get_jump_pose` goes over t (0 to 1) from standing to the airborne pose that ends it and is held.

    The airborne pose has one knee drawn up, the other leg trailing and the arms raised out for balance.

    Args:
        t: The progress through the clip.

    Returns:
        The bones' turns and the root's offset in metres.

    Example:
        >>> turns, offset = _get_jump_pose(1.0)
    """
    k: float = _get_ease(t)
    turns: dict[str, list[tuple[str, float]]] = {
        HIPS["L"]: [("x", -55.0 * k)],
        KNEES["L"]: [("x", 85.0 * k)],
        ANKLES["L"]: [("x", -20.0 * k)],
        HIPS["R"]: [("x", 15.0 * k)],
        KNEES["R"]: [("x", 35.0 * k)],
        ANKLES["R"]: [("x", 15.0 * k)],
        WAIST: [("x", 6.0 * k)],
        HEAD: [("x", -8.0 * k)],
    }
    for side, sign in SIDE_SIGNS.items():
        turns[SHOULDERS[side]] = [("y", sign * ARM_DOWN_DEG * (1.0 - 0.55 * k)), ("x", -20.0 * k)]
        turns[ELBOWS[side]] = _get_elbow_turns(side, IDLE_ELBOW_DEG + 30.0 * k)
    return turns, mathutils.Vector((0.0, 0.0, 0.0))


def _get_death_pose(t: float) -> Pose:
    """`_get_death_pose` goes over t (0 to 1) from standing to lying on the back, which ends it and is held.

    The knees buckle first (until t = 0.35), then the body tips back about the heels onto the ground, bouncing once.

    Args:
        t: The progress through the clip.

    Returns:
        The bones' turns and the root's offset in metres.

    Example:
        >>> turns, offset = _get_death_pose(1.0)
    """
    buckle: float = _get_ease(t / 0.35)
    fall: float = _get_ease((t - 0.25) / 0.5)
    bounce_deg: float = 6.0 * math.sin(math.pi * min(max((t - 0.75) / 0.2, 0.0), 1.0))
    sag_frac: float = buckle * (1.0 - fall)
    turns: dict[str, list[tuple[str, float]]] = {
        ROOT: [("x", -84.0 * fall + bounce_deg)],
        WAIST: [("x", 18.0 * sag_frac)],
        HEAD: [("x", -20.0 * fall), ("z", 25.0 * fall)],
    }
    for side, sign in SIDE_SIGNS.items():
        turns[HIPS[side]] = [("x", -40.0 * sag_frac - 10.0 * fall), ("y", sign * 8.0 * fall)]
        turns[KNEES[side]] = [("x", 75.0 * sag_frac + 15.0 * fall)]
        turns[ANKLES[side]] = [("x", -30.0 * sag_frac)]
        turns[SHOULDERS[side]] = [("y", sign * ARM_DOWN_DEG * (1.0 - 0.6 * fall)), ("x", -30.0 * buckle * (1.0 - fall))]
        turns[ELBOWS[side]] = _get_elbow_turns(side, IDLE_ELBOW_DEG + 25.0 * fall)
    sink_m: float = -0.16 * sag_frac + 0.12 * fall
    return turns, mathutils.Vector((0.0, 0.0, sink_m))


def _apply_pose(armature: bpy.types.Object, pose: Pose) -> None:
    """`_apply_pose` sets every bone of `armature` to `pose`, and every bone the pose doesn't name to rest.

    Each bone's turns are about axes of the rest pose and apply in order; the root's offset is in metres.

    Args:
        armature: The armature.
        pose: The bones' turns and the root's offset.

    Example:
        >>> _apply_pose(armature, _get_idle_pose(0.0))
    """
    turns, root_offset_m = pose
    for pose_bone in armature.pose.bones:
        pose_bone.rotation_mode = "QUATERNION"
        pose_bone.matrix_basis.identity()
    for bone_name, bone_turns in turns.items():
        pose_bone: bpy.types.PoseBone = armature.pose.bones[bone_name]
        rest_rotation: mathutils.Matrix = pose_bone.bone.matrix_local.to_3x3()
        rotation: mathutils.Quaternion = mathutils.Quaternion()
        for axis_name, turn_deg in bone_turns:
            rotation = mathutils.Quaternion(AXES[axis_name], math.radians(turn_deg)) @ rotation
        pose_bone.rotation_quaternion = (rest_rotation.inverted() @ rotation.to_matrix() @ rest_rotation).to_quaternion()
    root: bpy.types.PoseBone = armature.pose.bones[ROOT]
    root.location = root.bone.matrix_local.to_3x3().inverted() @ root_offset_m


def _add_clip(armature: bpy.types.Object, name: str, duration_s: float, get_pose: Callable[[float], Pose]) -> None:
    """`_add_clip` keyframes every frame of a clip `name`, `duration_s` long, from `get_pose`, as an action of its own.

    Args:
        armature: The armature.
        name: The clip's name in the glTF.
        duration_s: The clip's length in seconds; a looping clip's last frame equals its first.
        get_pose: The pose at a fraction (0 to 1) of the clip.

    Example:
        >>> _add_clip(armature, "Idle", IDLE_S, _get_idle_pose)
    """
    action: bpy.types.Action = bpy.data.actions.new(name)
    action.use_fake_user = True
    armature.animation_data_create().action = action
    frame_count: int = round(duration_s * FRAMES_PER_S)
    for frame in range(frame_count + 1):
        _apply_pose(armature, get_pose(frame / frame_count))
        for pose_bone in armature.pose.bones:
            pose_bone.keyframe_insert("rotation_quaternion", frame=frame)
            if pose_bone.name == ROOT:
                pose_bone.keyframe_insert("location", frame=frame)
    armature.animation_data.action = None


def main() -> None:
    """`main` makes the clean rig, adds the five clips and exports the model as glTF with separate textures.

    Example:
        >>> main()
    """
    source, folder = _get_argument_paths()
    armature: bpy.types.Object = _make_clean_rig(source)
    bpy.context.scene.render.fps = FRAMES_PER_S
    _add_clip(armature, "Idle", IDLE_S, _get_idle_pose)
    _add_clip(armature, "Walk", WALK_S, _get_walk_pose)
    _add_clip(armature, "Run", RUN_S, _get_run_pose)
    _add_clip(armature, "Jump", JUMP_S, _get_jump_pose)
    _add_clip(armature, "Death", DEATH_S, _get_death_pose)
    _apply_pose(armature, ({}, mathutils.Vector()))
    folder.mkdir(parents=True, exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=str(folder / "scene.gltf"),
        export_format="GLTF_SEPARATE",
        export_texture_dir=TEXTURE_FOLDER_NAME,
        export_animations=True,
        export_animation_mode="ACTIONS",
        export_force_sampling=True,
        export_def_bones=False,
        export_reset_pose_bones=True,
    )


if __name__ == "__main__":
    main()
