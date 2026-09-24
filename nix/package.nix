{
  lib,
  rustPlatform,
  makeWrapper,
  river,
  quickshell,
  libxkbcommon,
  swaylock,
  swayidle,
  wlopm,
  systemd,
  shellSrc,
}:

rustPlatform.buildRustPackage {
  pname = "mywm";
  version = "0.1.0";

  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: _type:
      !(builtins.elem (builtins.baseNameOf path) [ ".git" "target" "quickshell" ]);
  };

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [ makeWrapper ];
  nativeCheckInputs = [ libxkbcommon ];

  postPatch = ''
    substituteInPlace src/river.rs \
      --replace-fail /usr/share/river-protocols/stable ${river.src}/protocol
  '';

  postInstall = ''
    mkdir -p $out/share/mywm
    cp -r config scripts $out/share/mywm/
    cp -r ${shellSrc}/quickshell $out/share/mywm/quickshell

    wrapProgram $out/bin/mywm \
      --set-default MYWM_SHELL_DIR $out/share/mywm/quickshell \
      --prefix PATH : ${lib.makeBinPath [
        quickshell
        libxkbcommon
        swaylock
        swayidle
        wlopm
        systemd
      ]}
  '';

  meta = {
    description = "Custom River window manager with its Quickshell UI";
    homepage = "https://github.com/Chr1ssi/mywm";
    mainProgram = "mywm";
    platforms = lib.platforms.linux;
  };
}
