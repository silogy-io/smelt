
apt-get update && DEBIAN_FRONTEND=noninteractive apt-get -y install \
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

/usr/bin/python3 -m pip install --upgrade  --break-system-packages pip
/usr/bin/python3 -m pip install --upgrade  --break-system-packages maturin~=0.15
