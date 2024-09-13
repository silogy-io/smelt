
apt-get update && DEBIAN_FRONTEND=noninteractive apt-get -y install \
    clang \
    autoconf \
    automake \
    build-essential \
    cmake \
    git-core \
    pkg-config \
    texinfo \
    wget \
    yasm \
    zlib1g-dev \
    openssl \
    python3-dev \
    python3-pip \
    curl



curl -o rustup.sh --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs
sh rustup.sh -y
source $HOME/.cargo/env
rustup update
rustc -V

cargo install cargo-chef --locked

/usr/bin/python3 -m pip install --upgrade pip
/usr/bin/python3 -m pip install --upgrade maturin~=0.15
