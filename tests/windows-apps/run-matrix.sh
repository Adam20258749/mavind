#!/usr/bin/env bash
# Windows-app compatibility harness (STUB — Phase 3).
#
# Idea: given a list of portable .exe URLs + a launch check, boot the ISO in
# QEMU, install each with `mavind-wine install`, launch it, screenshot, and
# record pass/fail into a matrix. Not implemented yet.
#
# For now this documents the manual procedure so results are reproducible.
set -euo pipefail
cat <<'EOF'
Manual Windows-app test procedure
=================================
1. Boot Mavind (compat profile) in QEMU:
     tests/run-qemu.sh build/out/Mavind.iso --mem 3072
2. In the desktop, open a terminal (Super+Return) and:
     mavind-wine hint <appname>              # check the known verdict
     mavind-wine install ~/Downloads/App.exe --isolated
     mavind-wine run <appid>
3. Record: launched? window drew? basic interaction worked?
4. Add a row to compatibility/wine/hints.tsv if it differs from the seed.

Target set for the v0.1 matrix (docs/ROADMAP.md Phase 5):
  Notepad++, 7-Zip, IrfanView, Sumatra PDF, Foobar2000, PuTTY, WinRAR,
  Paint.NET, VLC (win), Notepad2, mIRC, HxD, Everything, Process Explorer,
  Audacity (win), GIMP (win), Blender (win), OBS (win), Inkscape (win), Krita (win)
EOF
