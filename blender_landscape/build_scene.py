"""Procedurally build a stylized 'gaming' landscape and render it.

Run with: blender -b --python build_scene.py
"""
import bpy
import bmesh
import math
import random
from mathutils import Vector, noise as mnoise

random.seed(7)

OUT_DIR = "/home/ec2-user/workspace/discord-dm/278326162995019777/blender_landscape"
RENDER_PATH = f"{OUT_DIR}/gaming_landscape.png"
BLEND_PATH = f"{OUT_DIR}/gaming_landscape.blend"

FAST_PREVIEW = False  # flip True for a quick low-res test render

# ---------------------------------------------------------------------------
# Reset scene
# ---------------------------------------------------------------------------
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.engine = 'BLENDER_EEVEE'


def link(obj):
    scene.collection.objects.link(obj)
    return obj


# ---------------------------------------------------------------------------
# Terrain - hand-rolled fractal height function for full amplitude control
# ---------------------------------------------------------------------------
SIZE = 70.0
HALF = SIZE / 2.0


def fbm(x, y, octaves, freq, gain=0.5, lac=2.0, off=0.0):
    amp = 1.0
    f = freq
    total = 0.0
    norm = 0.0
    for _ in range(octaves):
        total += amp * mnoise.noise(Vector((x * f + off, y * f + off, off)))
        norm += amp
        amp *= gain
        f *= lac
    return total / norm if norm else 0.0


def height_fn(x, y):
    macro = fbm(x, y, 3, 0.018, off=11.0)                      # broad rolling shape
    ridge_n = fbm(x, y, 4, 0.045, off=57.0)
    ridge = (1.0 - abs(ridge_n)) ** 2                            # sharp mountain ridges
    detail = fbm(x, y, 4, 0.22, off=131.0)

    h = macro * 4.5 + ridge * 11.0 + detail * 1.3

    dist = math.sqrt(x * x + y * y) / (HALF * 1.05)
    fade = max(0.0, 1.0 - dist)
    fade = fade ** 0.7
    return h * fade - 1.0


def slope_at(x, y, eps=0.4):
    dx = (height_fn(x + eps, y) - height_fn(x - eps, y)) / (2 * eps)
    dy = (height_fn(x, y + eps) - height_fn(x, y - eps)) / (2 * eps)
    return math.sqrt(dx * dx + dy * dy)


SEGMENTS = 220
bm = bmesh.new()
bmesh.ops.create_grid(bm, x_segments=SEGMENTS, y_segments=SEGMENTS, size=HALF)
min_z = float("inf")
max_z = float("-inf")
for v in bm.verts:
    z = height_fn(v.co.x, v.co.y)
    v.co.z = z
    min_z = min(min_z, z)
    max_z = max(max_z, z)

mesh = bpy.data.meshes.new("TerrainMesh")
bm.to_mesh(mesh)
bm.free()
terrain = bpy.data.objects.new("Terrain", mesh)
link(terrain)
bpy.context.view_layer.objects.active = terrain
terrain.select_set(True)

bpy.ops.object.shade_smooth()
print(f"TERRAIN HEIGHT RANGE: {min_z:.2f} .. {max_z:.2f}")

# ---------------------------------------------------------------------------
# Terrain material - height based grass/rock/snow
# ---------------------------------------------------------------------------
mat = bpy.data.materials.new("TerrainMat")
mat.use_nodes = True
nt = mat.node_tree
nt.nodes.clear()

out = nt.nodes.new("ShaderNodeOutputMaterial")
bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
bsdf.inputs["Roughness"].default_value = 0.85

geo = nt.nodes.new("ShaderNodeNewGeometry")
sep = nt.nodes.new("ShaderNodeSeparateXYZ")
maprange = nt.nodes.new("ShaderNodeMapRange")
maprange.inputs["From Min"].default_value = min_z
maprange.inputs["From Max"].default_value = max_z

noise = nt.nodes.new("ShaderNodeTexNoise")
noise.inputs["Scale"].default_value = 18.0
noise.inputs["Detail"].default_value = 3.0

mixfac = nt.nodes.new("ShaderNodeMath")
mixfac.operation = 'MULTIPLY'
mixfac.inputs[1].default_value = 0.06

addnode = nt.nodes.new("ShaderNodeMath")
addnode.operation = 'ADD'

ramp = nt.nodes.new("ShaderNodeValToRGB")
ramp.color_ramp.elements[0].position = 0.0
ramp.color_ramp.elements[0].color = (0.03, 0.09, 0.05, 1)  # dark shore/moss
ramp.color_ramp.elements[1].position = 1.0
ramp.color_ramp.elements[1].color = (0.95, 0.97, 1.0, 1)  # snow
e1 = ramp.color_ramp.elements.new(0.22)
e1.color = (0.09, 0.32, 0.10, 1)  # grass
e2 = ramp.color_ramp.elements.new(0.55)
e2.color = (0.30, 0.26, 0.20, 1)  # rock
e3 = ramp.color_ramp.elements.new(0.75)
e3.color = (0.45, 0.43, 0.40, 1)  # high rock

nt.links.new(geo.outputs["Position"], sep.inputs["Vector"])
nt.links.new(sep.outputs["Z"], maprange.inputs["Value"])
nt.links.new(maprange.outputs["Result"], addnode.inputs[0])
nt.links.new(noise.outputs["Fac"], mixfac.inputs[0])
nt.links.new(mixfac.outputs["Value"], addnode.inputs[1])
nt.links.new(addnode.outputs["Value"], ramp.inputs["Fac"])
nt.links.new(ramp.outputs["Color"], bsdf.inputs["Base Color"])
nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

terrain.data.materials.append(mat)

# ---------------------------------------------------------------------------
# World / sunset sky
# ---------------------------------------------------------------------------
world = bpy.data.worlds.new("GameSky")
scene.world = world
world.use_nodes = True
wnt = world.node_tree
wnt.nodes.clear()

wcoord = wnt.nodes.new("ShaderNodeTexCoord")
wsep = wnt.nodes.new("ShaderNodeSeparateXYZ")
wramp = wnt.nodes.new("ShaderNodeValToRGB")
wramp.color_ramp.elements[0].position = 0.0
wramp.color_ramp.elements[0].color = (0.95, 0.45, 0.22, 1)  # low horizon orange
wramp.color_ramp.elements[1].position = 1.0
wramp.color_ramp.elements[1].color = (0.05, 0.07, 0.22, 1)  # deep night blue
wg1 = wramp.color_ramp.elements.new(0.35)
wg1.color = (0.85, 0.42, 0.42, 1)  # pink
wg2 = wramp.color_ramp.elements.new(0.62)
wg2.color = (0.45, 0.30, 0.55, 1)  # purple
wbg = wnt.nodes.new("ShaderNodeBackground")
wbg.inputs["Strength"].default_value = 1.1
wout = wnt.nodes.new("ShaderNodeOutputWorld")

wnt.links.new(wcoord.outputs["Window"], wsep.inputs["Vector"])
wnt.links.new(wsep.outputs["Y"], wramp.inputs["Fac"])
wnt.links.new(wramp.outputs["Color"], wbg.inputs["Color"])
wnt.links.new(wbg.outputs["Background"], wout.inputs["Surface"])

# ---------------------------------------------------------------------------
# Lighting - warm key sun + cool blue fill
# ---------------------------------------------------------------------------
sun_data = bpy.data.lights.new("KeySun", type='SUN')
sun_data.energy = 3.2
sun_data.color = (1.0, 0.72, 0.45)
sun_data.angle = math.radians(2.0)
sun = bpy.data.objects.new("KeySun", sun_data)
link(sun)
sun.rotation_euler = (math.radians(78), 0, math.radians(35))

fill_data = bpy.data.lights.new("FillSun", type='SUN')
fill_data.energy = 0.6
fill_data.color = (0.35, 0.55, 1.0)
fill = bpy.data.objects.new("FillSun", fill_data)
link(fill)
fill.rotation_euler = (math.radians(35), 0, math.radians(-140))

# ---------------------------------------------------------------------------
# Low-poly pine tree (template, instanced via linked-data duplicates)
# ---------------------------------------------------------------------------
def make_tree_template():
    trunk_h = 0.9
    bpy.ops.mesh.primitive_cylinder_add(
        radius=0.09, depth=trunk_h, location=(0, 0, trunk_h / 2), vertices=6
    )
    trunk = bpy.context.active_object
    trunk.name = "TreeTrunk"

    cones = []
    base_z = trunk_h
    for i in range(3):
        h = 1.1 - i * 0.22
        r = 0.75 - i * 0.18
        z = base_z + i * 0.55 + h / 2
        bpy.ops.mesh.primitive_cone_add(
            radius1=r, radius2=0.02, depth=h, location=(0, 0, z), vertices=7
        )
        cone = bpy.context.active_object
        cones.append(cone)

    bpy.ops.object.select_all(action='DESELECT')
    trunk.select_set(True)
    for c in cones:
        c.select_set(True)
    bpy.context.view_layer.objects.active = trunk
    bpy.ops.object.join()
    tree = bpy.context.active_object
    tree.name = "TreeTemplate"

    trunk_mat = bpy.data.materials.new("TrunkMat")
    trunk_mat.diffuse_color = (0.12, 0.07, 0.04, 1)
    leaf_mat = bpy.data.materials.new("LeafMat")
    leaf_mat.diffuse_color = (0.05, 0.22, 0.09, 1)
    for m in (trunk_mat, leaf_mat):
        m.use_nodes = True
        m.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = m.diffuse_color
        m.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.9
    tree.data.materials.append(trunk_mat)
    tree.data.materials.append(leaf_mat)
    for poly in tree.data.polygons:
        poly.material_index = 0 if poly.center.z < trunk_h * 0.9 else 1

    scene.collection.objects.unlink(tree)
    return tree


def make_rock_template():
    bpy.ops.mesh.primitive_ico_sphere_add(radius=0.5, subdivisions=1, location=(0, 0, 0))
    rock = bpy.context.active_object
    rock.name = "RockTemplate"
    bm = bmesh.new()
    bm.from_mesh(rock.data)
    for v in bm.verts:
        v.co += Vector((random.uniform(-0.1, 0.1), random.uniform(-0.1, 0.1), random.uniform(-0.05, 0.05)))
    bm.to_mesh(rock.data)
    bm.free()
    rock.scale.z = 0.6

    rock_mat = bpy.data.materials.new("RockMat")
    rock_mat.use_nodes = True
    rock_mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (
        0.32, 0.31, 0.30, 1)
    rock_mat.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.95
    rock.data.materials.append(rock_mat)

    scene.collection.objects.unlink(rock)
    return rock


tree_template = make_tree_template()
rock_template = make_rock_template()

# ---------------------------------------------------------------------------
# Scatter trees & rocks across mid-height, gentle-slope terrain
# ---------------------------------------------------------------------------
range_h = max_z - min_z
tree_low = min_z + range_h * 0.10
tree_high = min_z + range_h * 0.58
rock_low = min_z + range_h * 0.42

placed_trees = 0
attempts = 0
place_half = HALF * 0.92
while placed_trees < 260 and attempts < 6000:
    attempts += 1
    x = random.uniform(-place_half, place_half)
    y = random.uniform(-place_half, place_half)
    z = height_fn(x, y)
    slope = slope_at(x, y)
    if tree_low < z < tree_high and slope < 0.9:
        inst = tree_template.copy()
        inst.data = tree_template.data
        inst.location = (x, y, z)
        inst.rotation_euler.z = random.uniform(0, math.tau)
        s = random.uniform(0.7, 1.5)
        inst.scale = (s, s, s * random.uniform(0.9, 1.2))
        link(inst)
        placed_trees += 1

placed_rocks = 0
attempts = 0
while placed_rocks < 70 and attempts < 3000:
    attempts += 1
    x = random.uniform(-place_half, place_half)
    y = random.uniform(-place_half, place_half)
    z = height_fn(x, y)
    if z > rock_low:
        inst = rock_template.copy()
        inst.data = rock_template.data
        inst.location = (x, y, z)
        inst.rotation_euler = (
            random.uniform(0, math.tau), random.uniform(0, math.tau), random.uniform(0, math.tau)
        )
        s = random.uniform(0.4, 1.6)
        inst.scale = (s, s, s)
        link(inst)
        placed_rocks += 1

# ---------------------------------------------------------------------------
# Lake in the lowest valley
# ---------------------------------------------------------------------------
best = None
for _ in range(3000):
    x = random.uniform(-place_half * 0.75, place_half * 0.75)
    y = random.uniform(-place_half * 0.75, place_half * 0.75)
    z = height_fn(x, y)
    if best is None or z < best[2]:
        best = (x, y, z)

lake_x, lake_y, lake_z = best
bpy.ops.mesh.primitive_circle_add(
    radius=7.5, fill_type='NGON', location=(lake_x, lake_y, lake_z + 0.05)
)
lake = bpy.context.active_object
lake.name = "Lake"
water_mat = bpy.data.materials.new("WaterMat")
water_mat.use_nodes = True
wb = water_mat.node_tree.nodes["Principled BSDF"]
wb.inputs["Base Color"].default_value = (0.02, 0.12, 0.22, 1)
wb.inputs["Roughness"].default_value = 0.03
if "Transmission Weight" in wb.inputs:
    wb.inputs["Transmission Weight"].default_value = 0.6
elif "Transmission" in wb.inputs:
    wb.inputs["Transmission"].default_value = 0.6
wb.inputs["IOR"].default_value = 1.33
lake.data.materials.append(water_mat)

# ---------------------------------------------------------------------------
# Camera - proper look-at aiming from a high, distant vantage point
# ---------------------------------------------------------------------------
def look_at(obj, target):
    direction = target - obj.location
    obj.rotation_euler = direction.to_track_quat('-Z', 'Y').to_euler()


cam_data = bpy.data.cameras.new("Cam")
cam_data.lens = 35
cam = bpy.data.objects.new("Cam", cam_data)
link(cam)

cam_height = max_z + max(16.0, range_h * 1.1)
cam_dist = HALF * 1.05
cam.location = Vector((-cam_dist * 0.85, -cam_dist, cam_height))
target = Vector((lake_x * 0.35, lake_y * 0.35, cam_height - cam_dist * 0.26))
look_at(cam, target)
scene.camera = cam

# Glowing sun disc near the horizon, straight ahead of the camera, for a
# cinematic sunset highlight (purely decorative - not tied to the SUN lamps).
forward = (target - cam.location).normalized()
sun_disc_pos = cam.location + forward * (cam_dist * 2.4)
sun_disc_pos.z = cam_height * 0.85
bpy.ops.mesh.primitive_uv_sphere_add(radius=6.0, location=sun_disc_pos)
sun_disc = bpy.context.active_object
sun_disc.name = "SunDisc"
disc_mat = bpy.data.materials.new("SunDiscMat")
disc_mat.use_nodes = True
emit = disc_mat.node_tree.nodes.new("ShaderNodeEmission")
emit.inputs["Color"].default_value = (1.0, 0.62, 0.28, 1)
emit.inputs["Strength"].default_value = 4.0
disc_out = disc_mat.node_tree.nodes["Material Output"]
disc_mat.node_tree.links.new(emit.outputs["Emission"], disc_out.inputs["Surface"])
sun_disc.data.materials.append(disc_mat)

# ---------------------------------------------------------------------------
# Render settings
# ---------------------------------------------------------------------------
eevee = scene.eevee
if hasattr(eevee, "use_bloom"):
    eevee.use_bloom = True
    eevee.bloom_intensity = 0.06
if hasattr(eevee, "use_ssr"):
    eevee.use_ssr = True
    eevee.use_ssr_refraction = True
if hasattr(eevee, "use_gtao"):
    eevee.use_gtao = True
if hasattr(eevee, "taa_render_samples"):
    eevee.taa_render_samples = 32 if not FAST_PREVIEW else 8

scene.render.resolution_x = 1920 if not FAST_PREVIEW else 640
scene.render.resolution_y = 1080 if not FAST_PREVIEW else 360
scene.render.image_settings.file_format = 'PNG'
scene.render.filepath = RENDER_PATH

bpy.ops.wm.save_as_mainfile(filepath=BLEND_PATH)
bpy.ops.render.render(write_still=True)
print(f"DONE trees={placed_trees} rocks={placed_rocks} lake=({lake_x:.1f},{lake_y:.1f},{lake_z:.1f})")
print(f"RENDER_SAVED:{RENDER_PATH}")
