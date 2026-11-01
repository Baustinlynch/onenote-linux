{
  description = "OneNote Linux desktop client";
  
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  
  outputs = { self, nixpkgs, rust-overlay }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };
      
      rustToolchain = pkgs.rust-bin.stable.latest.default;
      
      # Shared libraries the app needs at runtime (also used by the wrapper).
      runtimeLibs = with pkgs; [
        webkitgtk_4_1
        gtk4
        libayatana-appindicator
        libnotify
        glib
        glib-networking
        gdk-pixbuf
        cairo
        pango
        harfbuzz
        freetype
        fontconfig
        alsa-lib
        libpulseaudio
        libuuid
      ];
      
      onenote = pkgs.rustPlatform.buildRustPackage {
        pname = "onenote-linux";
        version = "0.2.1";
        src = ./.;
        
        cargoLock = {
          lockFile = ./Cargo.lock;
        };
        
        nativeBuildInputs = with pkgs; [
          pkg-config
          rustToolchain
        ];
        
        buildInputs = runtimeLibs ++ (with pkgs; [
          xdg-utils
          openssl
          libappindicator-gtk3
          shared-mime-info
          hicolor-icon-theme
        ]);
        
        # Skip tests during build (no headless WebKit in CI)
        doCheck = false;
        
        postInstall = ''
          mkdir -p $out/share/applications
          cp dist/onenote-linux.desktop $out/share/applications/
          
          mkdir -p $out/share/icons/hicolor/128x128/apps
          cp icons/128x128.png $out/share/icons/hicolor/128x128/apps/onenote-linux.png
          
          mkdir -p $out/share/icons/hicolor/32x32/apps
          cp icons/32x32.png $out/share/icons/hicolor/32x32/apps/onenote-linux.png
          
          # Wrap the binary so it can find the GTK/WebKit shared libraries
          # without being run inside a nix shell.
          mv $out/bin/onenote-linux $out/bin/.onenote-linux-unwrapped
          cat > $out/bin/onenote-linux << EOF
#!/usr/bin/env bash
export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath runtimeLibs}:\$LD_LIBRARY_PATH
export GIO_MODULE_DIR=${pkgs.glib-networking}/lib/gio/modules/
exec $out/bin/.onenote-linux-unwrapped "\$@"
EOF
          chmod +x $out/bin/onenote-linux
        '';
      };
      
      devShell = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkgs.cargo
          pkg-config
        ];
        
        buildInputs = with pkgs; [
          webkitgtk_4_1
          gtk4
          libayatana-appindicator
          libnotify
          glib-networking
          xdg-utils
          openssl
          
          # Additional GTK runtime dependencies
          glib
          gdk-pixbuf
          cairo
          pango
          harfbuzz
          freetype
          fontconfig
          alsa-lib
          libpulseaudio
          libappindicator-gtk3
          libuuid
          
          # Icons and other assets
          shared-mime-info
          hicolor-icon-theme
        ];
        
        shellHook = ''
          export GIO_MODULE_DIR=${pkgs.glib-networking}/lib/gio/modules/
          echo "OneNote Linux development shell"
          echo "To run the app: cargo run"
          echo "To build: cargo build --release"
          echo "To run tests: cargo test"
        '';
      };
      
    in
    {
      packages."${system}".default = onenote;
      devShells."${system}".default = devShell;
    };
}