docker build --platform linux/amd64  --build-arg SMELT_VERSION=$(bash test_utils/get_version.sh) -t smelt_dev . -f test_utils/Dockerfile.slurm_dev
docker run --hostname slurmctl --platform linux/amd64 smelt_dev
