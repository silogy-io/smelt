# small helper to setup centos8 machines 
sudo yum install python3.11 python3.11-pip python3.11-devel 
sudo alternatives --config python3

pip install maturin
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh


