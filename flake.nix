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
      
      buildInputs = with pkgs; [
        webkitgtk_4_1
        gtk4
        libayatana-appindicator
        libnotify
        xdg-utils
        pkg-config
        openssl
        gcc
        
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
      
      nativeBuildInputs = with pkgs; [
        (rust-bin.stable."1.85.0".default)
        pkgs.cargo
        pkg-config
      ];
      
      onenote = pkgs.stdenv.mkDerivation {
        pname = "onenote-linux";
        version = "0.2.1";
        src = ./.;
        
        inherit buildInputs nativeBuildInputs;
        
        buildPhase = ''
          export PATH=$rust/bin:$PATH
          cargo build --release
        '';
        
        installPhase = ''
          mkdir -p $out/bin
          cp target/release/onenote-linux $out/bin/
          
          mkdir -p $out/share/applications
          cp dist/onenote-linux.desktop $out/share/applications/
          
          mkdir -p $out/share/icons/hicolor/128x128/apps
          cp icons/128x128.png $out/share/icons/hicolor/128x128/apps/onenote-linux.png
          
          mkdir -p $out/share/icons/hicolor/32x32/apps
          cp icons/32x32.png $out/share/icons/hicolor/32x32/apps/onenote-linux.png
          
          mkdir -p $out/bin
          cat > $out/bin/onenote-linux << EOF
#!/usr/bin/env bash
export LD_LIBRARY_PATH=${pkgs.gtk4}/lib:${pkgs.webkitgtk_4_1}/lib:${pkgs.libayatana-appindicator}/lib:${pkgs.libpulseaudio}/lib
export GTK_PATH=${pkgs.gtk4}/lib/gtk-4.0

exec $rust/bin/cargo run --release -- \$@
EOF
          chmod +x $out/bin/onenote-linux
        '';
      };
      
      devShell = pkgs.mkShell {
        inherit buildInputs nativeBuildInputs;
        
        shellHook = ''
          echo "OneNote Linux development shell"
          echo "To run the app: $rust/bin/cargo run"
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