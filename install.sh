#!/bin/sh
# Compila e instala o Jaci para o usuário (padrão) ou em outro prefixo:
#   ./install.sh            -> ~/.local
#   sudo ./install.sh /usr  -> para todos
# Precisa de Rust (cargo) e Node.js 22 (npm).
set -e
cd "$(dirname "$0")"
prefix="${1:-$HOME/.local}"
id=io.github.DrakRosmann.Jaci

command -v cargo >/dev/null || { echo "Falta o Rust: https://rustup.rs"; exit 1; }
command -v npm >/dev/null || { echo "Falta o Node.js 22: https://nodejs.org"; exit 1; }

(cd web && npm ci --no-audit --no-fund && npm run build)
cargo build --release --bins

install -Dm0755 target/release/jaci "$prefix/bin/jaci"
install -Dm0755 target/release/computador-de-bordo "$prefix/bin/computador-de-bordo"
rm -rf "$prefix/share/jaci/web"
mkdir -p "$prefix/share/jaci"
cp -r web/dist "$prefix/share/jaci/web"
# Exemplos de missões em arquivos: copiados para a pasta do usuário na primeira vez.
rm -rf "$prefix/share/jaci/missoes"
cp -r missoes "$prefix/share/jaci/missoes"
install -Dm0644 web/public/jaci.svg "$prefix/share/icons/hicolor/scalable/apps/$id.svg"
mkdir -p "$prefix/share/applications"
cat > "$prefix/share/applications/$id.desktop" <<DESKTOP
[Desktop Entry]
Name=Jaci
Name[en]=Jaci
GenericName=Controle de missões espaciais
GenericName[en]=Space mission control
Comment=Lançamento de foguetes, cápsulas, estações, satélites e sondas
Comment[en]=Launch rockets, capsules, stations, satellites and probes
Exec=$prefix/bin/jaci --open
Icon=$id
Terminal=false
Type=Application
Categories=Education;Astronomy;
Keywords=foguete;espaço;órbita;satélite;rocket;space;orbit;
Actions=KSP;Parar;

[Desktop Action KSP]
Name=Jaci com o KSP
Name[en]=Jaci with KSP
Exec=$prefix/bin/jaci --open --ksp

[Desktop Action Parar]
Name=Parar o Jaci
Name[en]=Stop Jaci
Exec=pkill -x jaci
DESKTOP
command -v update-desktop-database >/dev/null && update-desktop-database "$prefix/share/applications" 2>/dev/null || true
echo "Instalado. Abra \"Jaci\" no menu de aplicativos, ou rode: jaci --open"
