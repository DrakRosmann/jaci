#!/bin/sh
# Compila os scripts do Jaci com o compilador do próprio kOS (sem abrir o KSP).
# Precisa do mono (mcs) e de um KSP com o kOS instalado:
#
#     ksp/verificar/verificar.sh [pasta do KSP]
set -e
cd "$(dirname "$0")"
KSP="${1:-$HOME/.steam/steam/steamapps/common/Kerbal Space Program}"
DLL="$KSP/GameData/kOS/Plugins/kOS.Safe.dll"
[ -f "$DLL" ] || { echo "não achei $DLL"; exit 1; }
OUT="${TMPDIR:-/tmp}/jaci-verificar"
mkdir -p "$OUT"
cp "$DLL" "$OUT/"
mcs -nologo verificar.cs -r:"$OUT/kOS.Safe.dll" -out:"$OUT/verificar.exe"
mono "$OUT/verificar.exe" ../Script/jaci/jaci.ks ../Script/boot/jaci.ks
