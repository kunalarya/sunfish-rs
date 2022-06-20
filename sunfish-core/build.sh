# Get config from first arg; default to debug.
CONFIG=${1:-"release"} 

if [[ "$CONFIG" == "debug" ]]; then
    cargo build
    rm -rf ~/Library/Audio/Plug-Ins/VST/Sunfish.vst/
    ./osx_vst_bundler.sh Sunfish ../target/debug/libsunfish.dylib && mv Sunfish.vst/ ~/Library/Audio/Plug-Ins/VST/
else
    cargo build --release
    rm -rf ~/Library/Audio/Plug-Ins/VST/Sunfish.vst/
    ./osx_vst_bundler.sh Sunfish ../target/release/libsunfish.dylib && mv Sunfish.vst/ ~/Library/Audio/Plug-Ins/VST/
fi
