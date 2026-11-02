"""Renders bean's scripted demo scenes and turns them into the README GIFs (docs/media/).

Run from the project root:  python scripts/make_gifs.py [scene ...]
Needs Pillow (pip install pillow). Frames go to target/demo_frames/.
"""
import os
import shutil
import subprocess
import sys

from PIL import Image

SCENES = ["punch", "slam", "tech", "gun"]
WIDTH = 480          # GIF width in pixels
FRAME_MS = 32        # bean saves one frame every 2 sim ticks (32 ms)
COLORS = 96          # palette size: smaller files, still clean on these flat colours


def render(scene, out_dir):
    shutil.rmtree(out_dir, ignore_errors=True)
    env = dict(os.environ, BEAN_DEMO=scene, BEAN_DEMO_OUT=out_dir)
    subprocess.run(["cargo", "run", "--release", "--quiet", "--bin", "bean"], env=env, check=True)


def to_gif(frame_dir, gif_path):
    files = sorted(f for f in os.listdir(frame_dir) if f.endswith(".png"))
    frames = []
    for f in files:
        im = Image.open(os.path.join(frame_dir, f)).convert("RGB")
        h = round(im.height * WIDTH / im.width)
        im = im.resize((WIDTH, h), Image.LANCZOS)
        frames.append(im.quantize(colors=COLORS, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE))
    frames[0].save(gif_path, save_all=True, append_images=frames[1:], duration=FRAME_MS, loop=0, optimize=True)
    return len(frames), os.path.getsize(gif_path)


def main():
    scenes = sys.argv[1:] or SCENES
    os.makedirs("docs/media", exist_ok=True)
    for s in scenes:
        frames = os.path.join("target", "demo_frames", s)
        render(s, frames)
        n, size = to_gif(frames, os.path.join("docs", "media", f"{s}.gif"))
        print(f"{s}: {n} frames, {size / 1024:.0f} KB")


if __name__ == "__main__":
    main()
