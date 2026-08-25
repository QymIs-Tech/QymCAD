# The BUILDER image for the Linux AppImage of QymCAD.
# The base is Ubuntu 22.04 (glibc 2.35) ON PURPOSE: the AppImage then starts on every distribution with
# glibc >= 2.35 (Ubuntu 22.04+, Debian 12+, Fedora 36+, a current Arch). Building on a newer distribution is
# not an option: glibc would come out newer than the one on a friend's 24.04 LTS and the binary would not
# start at all.
#
# OCCT 7.8.1 is built FROM SOURCE: apt on Ubuntu 22.04 carries only 7.5, while build.rs needs the merged
# modules of 7.8 (TKDESTEP and the like). Only the modelling modules (Draw and Visualization are off - the
# drawing here is our own, in egui/wgpu).
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential cmake git wget curl ca-certificates file patchelf zsync \
        librsvg2-bin desktop-file-utils \
        # dev headers for the GUI (egui/glow/winit) - the runtime .so files are gathered by linuxdeploy from ldd:
        libgl1-mesa-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libxext-dev \
        libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

# --- OCCT 7.8.1 from source -> /opt/occt (only the modules that are needed) ---
ARG OCCT_TAG=V7_8_1
RUN wget -q "https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/${OCCT_TAG}.tar.gz" -O /tmp/occt.tar.gz \
    && tar xf /tmp/occt.tar.gz -C /tmp \
    && cmake -S /tmp/OCCT-* -B /tmp/occt-build \
        -DCMAKE_BUILD_TYPE=Release \
        -DBUILD_LIBRARY_TYPE=Shared \
        -DINSTALL_DIR=/opt/occt \
        -DBUILD_MODULE_Draw=OFF \
        -DBUILD_MODULE_Visualization=OFF \
        -DBUILD_DOC_Overview=OFF \
        -DUSE_FREETYPE=OFF -DUSE_TK=OFF -DUSE_FREEIMAGE=OFF \
        -DUSE_RAPIDJSON=OFF -DUSE_DRACO=OFF -DUSE_OPENGL=OFF -DUSE_GLES2=OFF \
    && cmake --build /tmp/occt-build --target install -j"$(nproc)" \
    && rm -rf /tmp/occt* \
    && echo "/opt/occt/lib" > /etc/ld.so.conf.d/occt.conf && ldconfig

# --- Rust toolchain (pinned to the workspace rust-version = 1.85) ---
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.85.0 --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

# --- linuxdeploy + appimagetool (AppDir -> .AppImage; extract-and-run, so that FUSE is not needed in Docker) ---
RUN wget -q https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage -O /usr/local/bin/linuxdeploy \
    && wget -q https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage -O /usr/local/bin/appimagetool \
    && chmod +x /usr/local/bin/linuxdeploy /usr/local/bin/appimagetool
ENV APPIMAGE_EXTRACT_AND_RUN=1

# the OCCT paths for the build.rs of qymcad-kernel, plus the runtime linking during the build
ENV OCCT_INCLUDE_DIR=/opt/occt/include/opencascade \
    OCCT_LIB_DIR=/opt/occt/lib \
    LD_LIBRARY_PATH=/opt/occt/lib

WORKDIR /src
