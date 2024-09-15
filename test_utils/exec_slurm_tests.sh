docker build --platform linux/amd64  --build-arg SMELT_VERSION=$(bash test_utils/get_version.sh) -t smelt_dev . -f test_utils/Dockerfile.slurm_dev
docker run --hostname slurmctl  -v /var/run/docker.sock:/var/run/docker.sock --platform linux/amd64 smelt_dev
