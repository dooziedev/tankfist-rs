#!/bin/sh
# Pre-commit guard: refuses to commit 3D models, animations, textures or audio, and anything
# under assets/. Game assets from your own copy of Overwatch must stay on this PC.
# Installed by: git config core.hooksPath scripts/hooks
staged=$(git diff --cached --name-only --diff-filter=ACMR)
bad=$(printf '%s\n' "$staged" | grep -Ei '(^|/)assets/|\.(glb|gltf|bin|fbx|obj|dae|blend|blend1|owmdl|owanim|owmat|owentity|owmap|seanim|semodel|dds|tga|ktx2?|wem|ogg|wav)$')
pics=$(printf '%s\n' "$staged" | grep -Ei '\.(png|jpe?g)$' | grep -v '^docs/')
bad=$(printf '%s\n%s\n' "$bad" "$pics" | sed '/^$/d')
if [ -n "$bad" ]; then
  echo "Refusing to commit asset files (they must stay on this PC):"
  echo "$bad"
  exit 1
fi
