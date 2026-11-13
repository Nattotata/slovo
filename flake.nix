{
  description = "Slovo Dev Environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkg-config
            wrapGAppsHook4
            nodejs_22
            cargo
            rustc
            rustfmt
            clippy
            libiconv
          ];

          buildInputs = with pkgs; [
            at-spi2-atk
            atkmm
            cairo
            dbus
            gdk-pixbuf
            glib
            gtk3
            harfbuzz
            librsvg
            libsoup_3
            pango
            webkitgtk_4_1
            sqlite

            # GStreamer packages required by WebKitGTK runtime
            gst_all_1.gstreamer
            gst_all_1.gst-plugins-base
            gst_all_1.gst-plugins-good
          ];

          shellHook = ''
            export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-desktop-schemas:$XDG_DATA_DIRS"
            export GIO_EXTRA_MODULES="${pkgs.glib-networking}/lib/gio/modules"
            
            export PKG_CONFIG_PATH="${pkgs.webkitgtk_4_1}/lib/pkgconfig:${pkgs.libsoup_3}/lib/pkgconfig:${pkgs.dbus.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
            
            # Expose GStreamer plugins to the WebKitGTK webview
            export GST_PLUGIN_SYSTEM_PATH_1_0="${pkgs.gst_all_1.gst-plugins-base}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-good}/lib/gstreamer-1.0"
            
            echo "Slovo development shell initialized"
          '';
        };
      });
}
