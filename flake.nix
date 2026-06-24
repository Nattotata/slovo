{
  description = "Wordhoard dev environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        Cabal = pkgs.cabal-install;
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkg-config
            wrapGAppsHook4
            nodejs_22 # Explicit Node version for SvelteKit
            cargo
            rustc
            libiconv
          ];

          buildInputs = with pkgs; [
            at-spi2-atk
            atkmm
            cairo
            gdk-pixbuf
            glib
            gtk3
            harfbuzz
            librsvg
            libsoup_3
            pango
            webkitgtk_4_1
            sqlite # Local database utility
          ];

          shellHook = ''
            # Fixes missing schema issues on Wayland/Gnome
            export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-desktop-schemas:$XDG_DATA_DIRS"
            export GIO_EXTRA_MODULES="${pkgs.glib-networking}/lib/gio/modules"
            
            # Ensures rustc can find the system webkit libraries during compile
            export PKG_CONFIG_PATH="${pkgs.webkitgtk_4_1}/lib/pkgconfig:${pkgs.libsoup_3}/lib/pkgconfig"
            
            echo "⚡ Wordhoard development shell initialized"
          '';
        };
      });
}
